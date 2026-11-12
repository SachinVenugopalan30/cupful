<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Review — the confirm-before-save sheet for an estimate, a hand-entered meal, or an edit.
  Props:
    draft: Draft            the meal to review (copied on open; the caller's object isn't changed)
    title: string           sheet heading
    onClose: () => void     the sheet closed (saved or not); unmount it
    onSaved?: (queued: boolean) => void  the meal was saved; queued is true when it went to the
                            offline queue instead of the server
    onStale?: () => void    a newer write already won; refetch (nothing was saved)
  Saves the full meal with mealUpsert + sendOps, multiplier applied first (mealFromDraft). A write
  that lands in the offline queue counts as saved; then the "always use this" aliases (REST) run
  when online (a failure leaves the meal saved and says so), and are skipped when queued/offline.
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { errorMessage, foods, products, type SavedProduct } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { mealUpsert, sendOps } from '#lib/sync/index.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import { closeSheet } from '#lib/sheet.ts';
	import {
		MEAL_TYPES,
		MAX_DESCRIPTION,
		MULTIPLIERS,
		appendProduct,
		blankItem,
		capitalize,
		itemReady,
		mealFromDraft,
		sumItems,
		useProduct,
		type Draft
	} from './meal';
	import { aliasFailure, rememberAliases } from './review';
	import ReviewItem from './ReviewItem.svelte';
	import ReviewProductPicker from './ReviewProductPicker.svelte';

	let {
		draft,
		title,
		onClose,
		onSaved,
		onStale
	}: {
		draft: Draft;
		title: string;
		onClose: () => void;
		onSaved?: (queued: boolean) => void;
		onStale?: () => void;
	} =
		$props();

	let d = $state(structuredClone(untrack(() => $state.snapshot(draft)) as Draft));
	let dialog: HTMLDialogElement;
	let heading: HTMLHeadingElement;
	let returnFocus: HTMLElement | null = null;
	let saving = $state(false);
	let error = $state('');
	// The saved-product list for the whole sheet: every item shares it, so the picker doesn't fetch.
	let saved = $state<SavedProduct[] | null>(null);
	let savedError = $state('');

	const totals = $derived(sumItems(d.items, d.scale));
	const missing = $derived(d.items.filter((i) => !itemReady(i)).length);
	const uid = d.id.slice(-6);

	onMount(() => {
		returnFocus = document.activeElement as HTMLElement | null;
		dialog.showModal();
		heading?.focus();
		loadProducts();
	});

	/** The one saved-product list for the sheet. Retried from the picker when a load failed. */
	function loadProducts() {
		saved = null;
		savedError = '';
		products.list().then(
			(list) => (saved = list),
			(e) => (savedError = errorMessage(e))
		);
	}

	function close(force = false) {
		if (saving && !force) return;
		closeSheet(dialog, () => {
			returnFocus?.focus();
			onClose();
		});
	}

	// "Edit numbers" disclosures (items that need numbers show them always) and the product picker.
	let opened = $state<string[]>([]);
	const toggle = (id: string) => (opened = opened.includes(id) ? opened.filter((x) => x !== id) : [...opened, id]);
	let picking = $state<string | null>(null);
	const APPEND = 'append';

	function pick(id: string) {
		picking = picking === id ? null : id;
	}

	function addItem() {
		d.items.push(blankItem());
		// Focus the new item's name once it renders.
		queueMicrotask(() => dialog.querySelector<HTMLInputElement>('.item:last-child input')?.focus());
	}

	async function save() {
		if (saving || dialog.dataset.closing === 'true') return;
		// The parent may swap onSaved while this sheet is open; the save started under the old prop.
		const done = onSaved;
		error = '';
		if (!d.items.length) return void (error = 'Add at least one item.');
		if (missing) return void (error = 'Fill in the highlighted items first.');
		saving = true;
		try {
			const [r] = await sendOps([mealUpsert(mealFromDraft(d))]);
			const queued = r.status === 'queued';
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
				done?.(queued);
				close(true);
				return;
			}
			// Saved. Remember the phrases the user asked for; a failure keeps the sheet open to say so.
			const failed = await rememberAliases(d.items, foods.alias);
			done?.(queued);
			if (failed.length) {
				error = aliasFailure(failed);
				return;
			}
			close(true);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			if (dialog.dataset.closing !== 'true') saving = false;
		}
	}
</script>

<dialog
	class="sheet review"
	bind:this={dialog}
	oncancel={(event) => {
		event.preventDefault();
		close();
	}}
	aria-labelledby="review-title-{uid}"
>
	<form class="stack" method="dialog" onsubmit={(e) => (e.preventDefault(), save())} novalidate>
		<header class="sheet-head">
			<h2 id="review-title-{uid}" tabindex="-1" bind:this={heading}>{title}</h2>
			<button type="button" class="btn btn-quiet" onclick={() => close()} disabled={saving}>Cancel</button>
		</header>

		<fieldset class="segmented" aria-label="Meal">
			{#each MEAL_TYPES as t (t)}
				<label><input type="radio" name="review-type-{uid}" value={t} bind:group={d.meal_type} />{capitalize(t)}</label>
			{/each}
		</fieldset>

		<div class="field"><label for="review-name-{uid}">Name <span class="muted">(optional)</span></label><input id="review-name-{uid}" maxlength={MAX_DESCRIPTION} bind:value={d.description} /></div>

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
				<ReviewItem
					bind:item={d.items[n]}
					open={opened.includes(item.id)}
					picking={picking === item.id}
					{saved}
					{savedError}
					onRetry={loadProducts}
					onRemove={() => d.items.splice(n, 1)}
					onToggle={() => toggle(item.id)}
					onPick={() => pick(item.id)}
					onSwap={(p) => {
						d.items[n] = useProduct(d.items[n], p);
						picking = null;
					}}
				/>
			{/each}
		</ul>
		<div class="row actions">
			<button type="button" class="btn btn-quiet add" onclick={addItem}>Add item</button>
			<button type="button" class="btn" aria-expanded={picking === APPEND}
				aria-controls="picker-{APPEND}" onclick={() => pick(APPEND)}>+ Product</button>
		</div>
		{#if picking === APPEND}
			<ReviewProductPicker
				itemId={APPEND}
				mode="append"
				{saved}
				{savedError}
				onRetry={loadProducts}
				onPick={(p) => {
					d.items = appendProduct(d.items, p);
					picking = null;
				}}
			/>
		{/if}

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
		padding: var(--space-4);
		background: var(--surface-2);
		border-radius: var(--radius-lg);
	}
	.total-kcal {
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.02em;
	}
</style>
