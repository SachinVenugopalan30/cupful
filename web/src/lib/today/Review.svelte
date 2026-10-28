<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Review — the confirm-before-save sheet for an estimate, a hand-entered meal, or an edit.
  Props:
    draft: Draft            the meal to review (copied on open; the caller's object isn't changed)
    title: string           sheet heading
    onClose: () => void     the sheet closed (saved or not); unmount it
    onSaved: () => void     the meal was saved
  Saves the full meal with mealUpsert + sendOps, multiplier applied first (mealFromDraft).
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { errorMessage } from '#lib/api.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { mealUpsert, sendOps } from '#lib/sync/index.ts';
	import {
		MEAL_TYPES,
		MULTIPLIERS,
		blankItem,
		choose,
		itemReady,
		mealFromDraft,
		sumItems,
		withGrams,
		withValue,
		type Draft,
		type NumKey
	} from './meal';

	let { draft, title, onClose, onSaved }: { draft: Draft; title: string; onClose: () => void; onSaved: () => void } =
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
	const setValue = (n: number, k: NumKey, e: Event) => (d.items[n] = withValue(d.items[n], k, num(e)));

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
			onSaved();
			dialog.close();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			saving = false;
		}
	}

	const MACROS: [NumKey, string][] = [
		['kcal', 'kcal'],
		['protein_g', 'Protein g'],
		['carbs_g', 'Carbs g'],
		['fat_g', 'Fat g']
	];
	const label = (t: string) => t[0].toUpperCase() + t.slice(1);
</script>

<dialog class="sheet review" bind:this={dialog} onclose={onClose} aria-labelledby="review-title-{uid}">
	<form class="stack" method="dialog" onsubmit={(e) => (e.preventDefault(), save())} novalidate>
		<header class="sheet-head">
			<h2 id="review-title-{uid}">{title}</h2>
			<button type="button" class="btn btn-quiet" onclick={() => dialog.close()}>Cancel</button>
		</header>

		<fieldset class="segmented" aria-label="Meal">
			{#each MEAL_TYPES as t (t)}
				<label><input type="radio" name="review-type-{uid}" value={t} bind:group={d.meal_type} />{label(t)}</label>
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
						{#if !item.editable || item.options.length}
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
						<div class="field grams">
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
						{#if item.editable}
							{#each MACROS as [k, l] (k)}
								<div class="field">
									<label for="{k}-{item.id}">{l}</label>
									<input
										id="{k}-{item.id}"
										type="number"
										inputmode="decimal"
										min="0"
										step="any"
										value={item[k] ?? ''}
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
				Protein {formatGrams(totals.protein_g)} g · Carbs {formatGrams(totals.carbs_g)} g · Fat {formatGrams(
					totals.fat_g
				)} g
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
		background: var(--surface-2);
		border-radius: var(--radius-md);
	}
	.total-kcal {
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
	}
</style>
