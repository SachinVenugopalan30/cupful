<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Saved products, most recent first: search, tap to log again, remove. -->
<script lang="ts">
	import { errorMessage, products, type SavedProduct } from '../api';
	import { formatKcal } from '../format';
	import { filterSaved } from './logic';

	let { onPick, onScan }: { onPick: (p: SavedProduct) => void; onScan: () => void } = $props();

	// ponytail: fetched each time the list opens, no offline copy; M6's API cache covers offline.
	let list = $state<SavedProduct[] | null>(null);
	let error = $state('');
	let q = $state('');
	let confirming = $state<number | null>(null);
	const shown = $derived(list ? filterSaved(list, q) : []);

	async function load() {
		error = '';
		try {
			list = await products.list();
		} catch (e) {
			error = errorMessage(e, 'product');
		}
	}
	load();

	async function remove(p: SavedProduct) {
		error = '';
		try {
			await products.remove(p.food_id);
			list = list?.filter((x) => x.food_id !== p.food_id) ?? null;
			confirming = null;
		} catch (e) {
			error = errorMessage(e);
		}
	}

	const per = (p: SavedProduct) =>
		p.serving_g
			? `${formatKcal((p.kcal * p.serving_g) / 100)} kcal per serving`
			: `${formatKcal(p.kcal)} kcal per 100 ${p.serving_unit ?? 'g'}`;
</script>

<div class="stack" aria-busy={list === null && !error}>
	{#if error}
		<div class="notice notice-error" role="alert">
			<p>{error}</p>
			{#if list === null}<button type="button" class="btn" onclick={load}>Try again</button>{/if}
		</div>
	{/if}

	{#if list === null}
		{#if !error}
			<div class="skeleton" style="height: 2.75rem"></div>
			<div class="skeleton" style="height: 2.75rem"></div>
		{/if}
	{:else if list.length === 0}
		<div class="empty stack">
			<p>No saved products yet. Scan a barcode and log it, and it shows up here.</p>
			<button type="button" class="btn btn-primary" onclick={onScan}>Scan barcode</button>
		</div>
	{:else}
		<div class="field">
			<label for="saved-search" class="visually-hidden">Search saved products</label>
			<input id="saved-search" type="search" placeholder="Search by name or brand" bind:value={q} />
		</div>
		<ul class="list">
			{#each shown as p (p.food_id)}
				<li>
					{#if confirming === p.food_id}
						<p class="grow">Remove {p.name} from saved products?</p>
						<!-- svelte-ignore a11y_autofocus -->
						<button type="button" class="btn btn-danger" autofocus onclick={() => remove(p)}>Remove</button>
						<button type="button" class="btn btn-quiet" onclick={() => (confirming = null)}>Keep</button>
					{:else}
						<button type="button" class="pick grow" onclick={() => onPick(p)}>
							<span class="name">{p.name}</span>
							<span class="small muted">{p.brand ? `${p.brand} · ` : ''}{per(p)}</span>
						</button>
						<button type="button" class="btn btn-quiet" aria-label="Remove {p.name}" onclick={() => (confirming = p.food_id)}>
							Remove
						</button>
					{/if}
				</li>
			{:else}
				<li class="muted">No saved product matches “{q.trim()}”.</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.grow {
		flex: 1;
		min-width: 0;
	}
	li {
		flex-wrap: wrap;
	}
	.list > li {
		align-items: flex-start;
		padding: var(--space-3) 0;
	}
	.pick {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0.125rem;
		min-height: var(--touch);
		padding: var(--space-1) var(--space-2);
		margin-left: calc(-1 * var(--space-2));
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease),
			border-color var(--dur-fast) var(--ease);
	}
	.pick .name {
		font-weight: var(--weight-medium);
		overflow-wrap: anywhere;
	}
	.pick:hover {
		background: var(--surface-2);
		border-color: var(--border);
	}
	.pick:hover .name {
		color: var(--text);
	}
</style>
