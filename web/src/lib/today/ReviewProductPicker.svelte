<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ReviewProductPicker — the saved-product picker inside the review sheet, for "Use my product"
  (replace the item) and "+ Product" (append a new item). Loads the saved products when it mounts,
  filters them by name, and reports the chosen product to the parent. Scanning and label entry run
  inline here, so the review sheet stays the only dialog; leaving the scan view stops the camera.
  Props:
    itemId: string                               ids for the inputs and the picker region
    mode: 'replace' | 'append'                   what the parent does with the pick
    onPick: (p: Product) => void                 the user picked (or scanned) a product
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { errorMessage, products, type Product, type SavedProduct } from '#lib/api.ts';
	import { filterSaved } from '#lib/products/logic.ts';
	import LabelForm from '#lib/products/LabelForm.svelte';
	import ScanView from '#lib/products/ScanView.svelte';

	let {
		itemId,
		mode,
		onPick
	}: { itemId: string; mode: 'replace' | 'append'; onPick: (p: Product) => void } = $props();

	let query = $state('');
	let saved = $state<SavedProduct[] | null>(null);
	let savedError = $state('');
	let view = $state<'list' | 'scan' | { label: string }>('list');
	const matches = $derived(filterSaved(saved ?? [], query));

	onMount(() => {
		products.list().then(
			(l) => (saved = l),
			(e) => (savedError = errorMessage(e))
		);
	});
</script>

<div class="picker stack" id="picker-{itemId}">
	{#if view === 'scan'}
		<ScanView
			onFound={onPick}
			onLabel={(barcode) => (view = { label: barcode })}
			onSaved={() => (view = 'list')}
		/>
	{:else if typeof view === 'object'}
		<LabelForm barcode={view.label} onSaved={onPick} />
		<button type="button" class="btn btn-quiet" onclick={() => (view = 'scan')}>Back to scanning</button>
	{:else}
		{#if savedError}
			<p class="notice notice-error" role="alert">{savedError}</p>
		{:else if saved === null}
			<p class="small muted" role="status">Loading…</p>
		{:else if saved.length === 0}
			<p class="small muted">No saved products yet. Scan a barcode to add one.</p>
		{:else}
			<div class="field">
				<label for="search-{itemId}">Search my products</label>
				<input id="search-{itemId}" type="search" bind:value={query} />
			</div>
			<ul class="list">
				{#each matches as p (p.food_id)}
					<li>
						<button type="button" class="btn btn-quiet pick" onclick={() => onPick(p)}>
							<span>{p.name}</span>
							{#if p.brand}<span class="muted small">{p.brand}</span>{/if}
						</button>
					</li>
				{:else}
					<li class="small muted">No match.</li>
				{/each}
			</ul>
		{/if}
		<button type="button" class="btn btn-quiet scan" onclick={() => (view = 'scan')}>
			{mode === 'append' ? '+ Scan a product' : 'Scan a product'}
		</button>
	{/if}
</div>

<style>
	.picker {
		padding: var(--space-3) var(--space-4);
		background: var(--surface-2);
		border-radius: var(--radius-md);
	}
	.pick {
		flex: 1;
		justify-content: flex-start;
		gap: var(--space-2);
		text-align: left;
	}
	.scan {
		align-self: flex-start;
	}
</style>
