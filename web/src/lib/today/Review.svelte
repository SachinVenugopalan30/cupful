<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Review — the confirm-before-save sheet for an estimate, a hand-entered meal, or an edit.
  Props:
    draft: Draft            the meal to review (copied on open; the caller's object isn't changed)
    title: string           sheet heading
    onClose: () => void     the sheet closed (saved or not); unmount it
    onSaved: () => void     the meal was saved
    onStale?: () => void    a newer write already won; refetch (nothing was saved)
  Saves the full meal with mealUpsert + sendOps, multiplier applied first (mealFromDraft). A write
  that lands in the offline queue counts as saved; then the "always use this" aliases (REST) run
  when online (a failure leaves the meal saved and says so), and are skipped when queued/offline.
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { errorMessage, foods, products, type SavedProduct } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { filterSaved } from '#lib/products/logic.ts';
	import { formatGrams, formatKcal, round } from '#lib/format.ts';
	import { mealUpsert, sendOps } from '#lib/sync/index.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import {
		MEAL_TYPES,
		MULTIPLIERS,
		blankItem,
		capitalize,
		choose,
		itemReady,
		mealFromDraft,
		useProduct,
		sumItems,
		withGrams,
		withKcal,
		withValue,
		type Draft,
		type NumKey
	} from './meal';
	import { MACROS, aliasFailure, rememberAliases } from './review';

	let {
		draft,
		title,
		onClose,
		onSaved,
		onStale
	}: { draft: Draft; title: string; onClose: () => void; onSaved: () => void; onStale?: () => void } =
		$props();

	let d = $state(structuredClone(untrack(() => $state.snapshot(draft)) as Draft));
	let dialog: HTMLDialogElement;
	let saving = $state(false);
	let error = $state('');

	const totals = $derived(sumItems(d.items, d.scale));
	const missing = $derived(d.items.filter((i) => !itemReady(i)).length);
	const uid = d.id.slice(-6);

	onMount(() => dialog.showModal());

	const num = (e: Event) => {
		const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
		return Number.isFinite(v) && v >= 0 ? v : null;
	};
	function setGrams(n: number, e: Event) {
		const g = num(e);
		if (g !== null && g > 0) d.items[n] = withGrams(d.items[n], g);
		else if (d.items[n].editable) d.items[n].grams = null;
	}
	const setValue = (n: number, k: NumKey, e: Event) =>
		(d.items[n] = k === 'kcal' ? withKcal(d.items[n], num(e)) : withValue(d.items[n], k, num(e)));

	// "Edit numbers" disclosures (items that need numbers show them always) and the product picker.
	let opened = $state<string[]>([]);
	const toggle = (id: string) => (opened = opened.includes(id) ? opened.filter((x) => x !== id) : [...opened, id]);
	let picking = $state<string | null>(null);
	let query = $state('');
	let saved = $state<SavedProduct[] | null>(null);
	let savedError = $state('');
	const matches = $derived(filterSaved(saved ?? [], query));

	function pick(id: string) {
		query = '';
		picking = picking === id ? null : id;
		if (saved === null && !savedError) {
			products.list().then(
				(l) => (saved = l),
				(e) => (savedError = errorMessage(e))
			);
		}
	}
	function swap(n: number, p: SavedProduct) {
		d.items[n] = useProduct(d.items[n], p);
		picking = null;
	}

	function addItem() {
		d.items.push(blankItem());
		// Focus the new item's name once it renders.
		queueMicrotask(() => dialog.querySelector<HTMLInputElement>('.item:last-child input')?.focus());
	}

	async function save() {
		error = '';
		if (!d.items.length) return void (error = 'Add at least one item.');
		if (missing) return void (error = 'Fill in the highlighted items first.');
		saving = true;
		try {
			const [r] = await sendOps([mealUpsert(mealFromDraft(d))]);
			if (r.status === 'rejected') {
				error = `The meal wasn't saved: ${r.error ?? 'the server rejected it'}.`;
				return;
			}
			if (r.status === 'stale') {
				error = staleNotice('meal');
				onStale?.();
				return;
			}
			// Queued is saved. Aliases are online-only configuration, so close without writing them.
			if (r.status === 'queued' || !connection.online) {
				onSaved();
				dialog.close();
				return;
			}
			// Saved. Remember the phrases the user asked for; a failure keeps the sheet open to say so.
			const failed = await rememberAliases(d.items, foods.alias);
			onSaved();
			if (failed.length) {
				error = aliasFailure(failed);
				return;
			}
			dialog.close();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			saving = false;
		}
	}
</script>

<dialog class="sheet review" bind:this={dialog} onclose={onClose} aria-labelledby="review-title-{uid}">
	<form class="stack" method="dialog" onsubmit={(e) => (e.preventDefault(), save())} novalidate>
		<header class="sheet-head">
			<h2 id="review-title-{uid}">{title}</h2>
			<button type="button" class="btn btn-quiet" onclick={() => dialog.close()}>Cancel</button>
		</header>

		<fieldset class="segmented" aria-label="Meal">
			{#each MEAL_TYPES as t (t)}
				<label><input type="radio" name="review-type-{uid}" value={t} bind:group={d.meal_type} />{capitalize(t)}</label>
			{/each}
		</fieldset>

		{#if d.warnings.length}
			<div class="notice notice-warning">
				{#each d.warnings as w, i (i)}<p>{w}</p>{/each}
			</div>
		{/if}
		{#if d.assumptions || d.confidence}
			<p class="notice notice-info">
				{#if d.assumptions}<span>Assumed: {d.assumptions}</span>{/if}
				{#if d.confidence}<span class="small">Confidence: {d.confidence}</span>{/if}
			</p>
		{/if}

		<ul class="items">
			{#each d.items as item, n (item.id)}
				{@const ready = itemReady(item)}
				<li class="item" class:needs={!ready}>
					<div class="item-head">
						{#if item.editable}
							<label class="visually-hidden" for="name-{item.id}">Item name</label>
							<input
								id="name-{item.id}"
								class="name-input"
								placeholder="Food name"
								bind:value={d.items[n].name}
								aria-invalid={!item.name.trim()}
							/>
						{:else}
							<h3 class="name">{item.name}</h3>
						{/if}
						<button
							type="button"
							class="btn btn-quiet remove"
							aria-label="Remove {item.name || 'item'}"
							onclick={() => d.items.splice(n, 1)}
						>
							<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
						</button>
					</div>
					<p class="row meta small">
						{#if item.portion_text}<span class="muted">{item.portion_text}</span>{/if}
						{#if item.origin === 'manual'}
							<span class="chip">{item.food_id === null ? 'By hand' : 'Edited'}</span>
						{:else if !item.editable || item.options.length}
							{#if item.origin === 'food_db'}
								<span class="chip chip-db">Database</span>
							{:else}
								<span class="chip chip-ai">AI estimate</span>
							{/if}
						{/if}
						{#if !ready}<span class="chip chip-warning">Needs your numbers</span>{/if}
					</p>

					{#if item.options.length}
						<fieldset class="choice">
							<legend class="small">Which {item.options[0].name.toLowerCase()}?</legend>
							{#each item.options as o, k (k)}
								<label class="row">
									<input
										type="radio"
										name="choice-{item.id}"
										checked={item.chosen === k}
										onchange={() => (d.items[n] = choose(d.items[n], k))}
									/>
									<span>
										<span class="opt">{o.name}</span>
										<span class="muted small">· {o.origin === 'food_db' ? 'Database' : 'AI estimate'}</span>
									</span>
								</label>
							{/each}
						</fieldset>
					{/if}

					<div class="nums">
						<div class="field">
							<label for="grams-{item.id}">Grams</label>
							<input
								id="grams-{item.id}"
								type="number"
								inputmode="decimal"
								min="0"
								step="any"
								value={item.grams === null ? '' : Number(formatGrams(item.grams))}
								oninput={(e) => setGrams(n, e)}
							/>
						</div>
						{#if item.editable || opened.includes(item.id)}
							{#each MACROS as [k, l] (k)}
								<div class="field">
									<label for="{k}-{item.id}">{l}</label>
									<input
										id="{k}-{item.id}"
										type="number"
										inputmode="decimal"
										min="0"
										step="any"
										value={item[k] === null ? '' : round(item[k], 2)}
										oninput={(e) => setValue(n, k, e)}
										aria-invalid={item[k] === null}
									/>
								</div>
							{/each}
						{:else}
							<p class="values small">
								<span class="num kcal">{formatKcal(item.kcal ?? 0)} kcal</span>
								<span class="muted num"
									>P {formatGrams(item.protein_g ?? 0)} · C {formatGrams(item.carbs_g ?? 0)} · F {formatGrams(
										item.fat_g ?? 0
									)} g</span
								>
							</p>
						{/if}
					</div>

					<div class="row actions">
						{#if !item.editable}
							<button
								type="button"
								class="btn btn-quiet"
								aria-expanded={opened.includes(item.id)}
								aria-controls="kcal-{item.id}"
								onclick={() => toggle(item.id)}>Edit numbers</button
							>
						{/if}
						<button
							type="button"
							class="btn btn-coral"
							aria-expanded={picking === item.id}
							aria-controls="picker-{item.id}"
							onclick={() => pick(item.id)}>Use my product</button
						>
					</div>

					{#if picking === item.id}
						<div class="picker stack" id="picker-{item.id}">
							{#if savedError}
								<p class="notice notice-error" role="alert">{savedError}</p>
							{:else if saved === null}
								<p class="small muted" role="status">Loading…</p>
							{:else if saved.length === 0}
								<p class="small muted">No saved products yet. Scan a barcode to add one.</p>
							{:else}
								<div class="field">
									<label for="search-{item.id}">Search my products</label>
									<input id="search-{item.id}" type="search" bind:value={query} />
								</div>
								<ul class="list">
									{#each matches as p (p.food_id)}
										<li>
											<button type="button" class="btn btn-quiet pick" onclick={() => swap(n, p)}>
												<span>{p.name}</span>
												{#if p.brand}<span class="muted small">{p.brand}</span>{/if}
											</button>
										</li>
									{:else}
										<li class="small muted">No match.</li>
									{/each}
								</ul>
							{/if}
						</div>
					{/if}

					{#if item.swapped && item.phrase.trim()}
						<label class="row">
							<input type="checkbox" bind:checked={d.items[n].remember} />
							<span>Always use this for '{item.phrase}'</span>
						</label>
					{/if}
				</li>
			{/each}
		</ul>
		<button type="button" class="btn btn-quiet add" onclick={addItem}>Add item</button>

		<fieldset class="field">
			<legend class="label">Portion of the whole meal</legend>
			<div class="segmented">
				{#each MULTIPLIERS as m (m)}
					<label><input type="radio" name="scale-{uid}" value={m} bind:group={d.scale} />×{m}</label>
				{/each}
			</div>
		</fieldset>

		<div class="totals" aria-live="polite">
			<p><span class="small muted">Total{d.scale !== 1 ? ` (×${d.scale})` : ''}</span></p>
			<p class="num total-kcal">{formatKcal(totals.kcal)} <span class="small">kcal</span></p>
			<p class="small num muted">
				<span class="m-protein">Protein {formatGrams(totals.protein_g)} g</span> ·
				<span class="m-carbs">Carbs {formatGrams(totals.carbs_g)} g</span> ·
				<span class="m-fat">Fat {formatGrams(totals.fat_g)} g</span>
			</p>
		</div>

		{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}

		<button class="btn btn-primary btn-block" disabled={saving}>{saving ? 'Saving…' : 'Save meal'}</button>
	</form>
</dialog>

<style>
	.review form {
		gap: var(--space-4);
	}
	.items {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
	}
	.item.needs {
		border-color: var(--warning);
		box-shadow: inset 0 0 0 1px var(--warning);
	}
	.item-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.name {
		flex: 1;
		font-size: var(--text-md);
	}
	.name::first-letter,
	.opt::first-letter {
		text-transform: uppercase;
	}
	.opt {
		display: inline-block;
	}
	.name-input {
		flex: 1;
	}
	.remove svg,
	.remove {
		flex: none;
	}
	.remove svg {
		width: 1.25rem;
		height: 1.25rem;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
	}
	.meta {
		gap: var(--space-2);
	}
	.choice {
		border: 0;
		margin: 0;
		padding: var(--space-2) var(--space-3);
		background: var(--surface-2);
		border-radius: var(--radius-md);
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.choice legend {
		font-weight: var(--weight-medium);
		float: left;
		width: 100%;
		margin-bottom: var(--space-1);
	}
	.choice label {
		min-height: var(--touch);
		flex-wrap: nowrap;
	}
	.nums {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(5.5rem, 1fr));
		gap: var(--space-2);
		align-items: end;
	}
	.values {
		grid-column: 2 / -1;
		display: flex;
		flex-direction: column;
		padding-bottom: var(--space-1);
	}
	.kcal {
		font-size: var(--text-md);
		font-weight: var(--weight-bold);
	}
	.actions {
		gap: var(--space-1);
	}
	.m-protein {
		color: var(--protein-text);
	}
	.m-carbs {
		color: var(--carbs-text);
	}
	.m-fat {
		color: var(--fat-text);
	}
	.picker {
		padding: var(--space-3);
		background: var(--coral-soft);
		border-radius: var(--radius-md);
	}
	.pick {
		flex: 1;
		justify-content: flex-start;
		gap: var(--space-2);
		text-align: left;
	}
	.add {
		align-self: flex-start;
	}
	fieldset.field {
		border: 0;
		margin: 0;
		padding: 0;
	}
	.segmented label {
		padding: 0 var(--space-1);
	}
	.totals {
		padding: var(--space-3) var(--space-4);
		background: var(--sand);
		border-radius: var(--radius-md);
	}
	.total-kcal {
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
	}
</style>
