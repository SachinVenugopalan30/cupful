<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  PhraseLinks — "Use for descriptions": the words that make an estimate pick this product.
  Props: product: Product   the product being viewed
  Reads and writes aliases over REST (online only); past meals never change.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { errorMessage, foods, type AliasRow, type Product } from '../api';
	import { sortPhrases } from './logic';

	let { product: p }: { product: Product } = $props();

	let rows = $state<AliasRow[] | null>(null);
	let phrase = $state('');
	let busy = $state(false);
	let error = $state('');

	const lists = $derived(sortPhrases(rows ?? [], p.name, p.food_id));

	async function load() {
		try {
			rows = await foods.aliases();
		} catch (e) {
			error = errorMessage(e);
		}
	}
	onMount(load);

	/** Run a change, then reload the list; true if it worked. */
	async function change(fn: () => Promise<unknown>) {
		busy = true;
		error = '';
		try {
			await fn();
			await load();
			return true;
		} catch (e) {
			error = errorMessage(e);
			return false;
		} finally {
			busy = false;
		}
	}

	async function add() {
		const alias = phrase.trim();
		if (alias && (await change(() => foods.alias(alias, p.food_id)))) phrase = '';
	}
</script>

<section class="stack phrases" aria-labelledby="phrases-title">
	<h3 id="phrases-title">Use for descriptions</h3>
	<p class="small muted">Descriptions with these words use this product. Past meals don't change.</p>

	{#if rows === null && !error}
		<p class="small muted" role="status">Loading…</p>
	{/if}

	{#if lists.mine.length}
		<ul class="list">
			{#each lists.mine as a (a.id)}
				<li>
					<span>{a.alias}</span>
					<button
						type="button"
						class="btn btn-quiet"
						disabled={busy}
						aria-label="Remove {a.alias}"
						onclick={() => change(() => foods.removeAlias(a.id))}>Remove</button
					>
				</li>
			{/each}
		</ul>
	{/if}

	{#if lists.others.length}
		<ul class="list">
			{#each lists.others as a (a.id)}
				<li>
					<span class="small">'{a.alias}' → {a.food_name}</span>
					<button
						type="button"
						class="btn"
						disabled={busy}
						aria-label="Use this product instead for {a.alias}"
						onclick={() => change(() => foods.alias(a.alias, p.food_id))}>Use this product instead</button
					>
				</li>
			{/each}
		</ul>
	{/if}

	<form class="add row" onsubmit={(e) => (e.preventDefault(), add())}>
		<div class="field">
			<label for="phrase-add">Add a phrase</label>
			<input id="phrase-add" bind:value={phrase} maxlength="100" placeholder="e.g. chocolate protein powder" />
		</div>
		<button class="btn" disabled={busy || !phrase.trim()}>Add phrase</button>
	</form>

	{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}
</section>

<style>
	.phrases {
		border-top: 1px solid var(--border);
		padding-top: var(--space-4);
	}
	.list > li {
		justify-content: space-between;
	}
	.add {
		align-items: end;
		flex-wrap: nowrap;
	}
	.add .field {
		flex: 1;
		min-width: 0;
	}
</style>
