<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ReviewProductPicker — the "Use my product" picker inside the review sheet. Loads the saved
  products when it mounts, filters them by name, and reports the chosen product to the parent.
  Props:
    itemId: string                    the meal item the picker is open for (ids and labels)
    onSwap: (p: SavedProduct) => void the user picked a saved product
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { errorMessage, products, type SavedProduct } from '#lib/api.ts';
	import { filterSaved } from '#lib/products/logic.ts';

	let {
		itemId,
		onSwap
	}: { itemId: string; onSwap: (p: SavedProduct) => void } = $props();

	let query = $state('');
	let saved = $state<SavedProduct[] | null>(null);
	let savedError = $state('');
	const matches = $derived(filterSaved(saved ?? [], query));

	onMount(() => {
		products.list().then(
			(l) => (saved = l),
			(e) => (savedError = errorMessage(e))
		);
	});
</script>

<div class="picker stack" id="picker-{itemId}">
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
					<button type="button" class="btn btn-quiet pick" onclick={() => onSwap(p)}>
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
</style>
