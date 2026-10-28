<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  BarcodeEntry — "Scan barcode" and "Saved products". One sheet (native <dialog>) moves between
  scan, label entry, product and saved views; closing it unmounts the scanner, which stops the camera.
  Props (fixed contract; Today page passes these):
    date: string          YYYY-MM-DD the meal is logged on
    onSaved: () => void   called after sendOps reports the meal applied
-->
<script lang="ts">
	import { tick } from 'svelte';
	import type { Product } from '../api';
	import LabelForm from './LabelForm.svelte';
	import ProductView from './ProductView.svelte';
	import SavedList from './SavedList.svelte';
	import ScanView from './ScanView.svelte';

	let { date, onSaved }: { date: string; onSaved: () => void } = $props();

	type View =
		| { name: 'scan' }
		| { name: 'label'; barcode: string }
		| { name: 'product'; product: Product; servings: number | null }
		| { name: 'saved' };

	let dialog = $state<HTMLDialogElement>();
	let heading = $state<HTMLHeadingElement>();
	let view = $state<View | null>(null);
	let announce = $state('');

	const title = $derived(
		view?.name === 'product'
			? view.product.name
			: { scan: 'Scan barcode', label: 'Enter from the label', saved: 'Saved products' }[view?.name ?? 'scan']
	);

	async function show(v: View) {
		view = v;
		announce = '';
		if (!dialog?.open) dialog?.showModal();
		await tick();
		heading?.focus();
	}

	function added() {
		const name = view?.name === 'product' ? view.product.name : '';
		dialog?.close();
		announce = `Added ${name}.`;
		onSaved();
	}
</script>

<section class="entry" aria-label="Packaged food">
	<div class="buttons">
		<button type="button" class="btn" onclick={() => show({ name: 'scan' })}>
			<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7V4h3M18 4h3v3M21 17v3h-3M6 20H3v-3M7 8v8M10 8v8M13 8v8M16 8v8" /></svg>
			Scan barcode
		</button>
		<button type="button" class="btn" onclick={() => show({ name: 'saved' })}>Saved products</button>
	</div>
	<p class="visually-hidden" aria-live="polite">{announce}</p>
</section>

<dialog class="sheet" bind:this={dialog} aria-labelledby="product-sheet-title" onclose={() => (view = null)}>
	{#if view}
		<header class="sheet-head">
			<h2 id="product-sheet-title" tabindex="-1" bind:this={heading}>{title}</h2>
			<button type="button" class="btn btn-quiet close" aria-label="Close" onclick={() => dialog?.close()}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
			</button>
		</header>
		{#if view.name === 'scan'}
			<ScanView
				onFound={(product) => show({ name: 'product', product, servings: null })}
				onLabel={(barcode) => show({ name: 'label', barcode })}
				onSaved={() => show({ name: 'saved' })}
			/>
		{:else if view.name === 'label'}
			<LabelForm barcode={view.barcode} onSaved={(product) => show({ name: 'product', product, servings: null })} />
		{:else if view.name === 'product'}
			{#key view.product}
				<ProductView product={view.product} servings={view.servings} {date} onAdded={added} />
			{/key}
		{:else}
			<SavedList
				onPick={(p) => show({ name: 'product', product: p, servings: p.last_servings })}
				onScan={() => show({ name: 'scan' })}
			/>
		{/if}
	{/if}
</dialog>

<style>
	.buttons {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-2);
	}
	.buttons .btn {
		white-space: nowrap;
		padding: 0 var(--space-2);
	}
	svg {
		width: 1.25rem;
		height: 1.25rem;
		flex: none;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.sheet-head {
		margin-bottom: var(--space-3);
	}
	.close {
		margin-right: calc(-1 * var(--space-2));
	}
</style>
