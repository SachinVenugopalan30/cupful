<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- One item row in Review: name, origin, number inputs, household unit picker and the product swap. -->
<script lang="ts">
	import { formatGrams, formatKcal, round } from '#lib/format.ts';
	import type { Product, SavedProduct } from '#lib/api.ts';
	import { matchProduct } from './productMatch';
	import ReviewProductPicker from './ReviewProductPicker.svelte';
	import { MACROS, stepQuantity } from './review';
	import {
		choose,
		itemReady,
		withGramsInput,
		withKcal,
		withQuantity,
		withValue,
		type DraftItem,
		type NumKey
	} from './meal';

	let {
		item = $bindable(),
		open,
		picking,
		saved = null,
		savedError = '',
		onRetry,
		onRemove,
		onToggle,
		onPick,
		onSwap
	}: {
		item: DraftItem;
		open: boolean;
		picking: boolean;
		/** The review sheet's saved products, loaded once for every row. */
		saved?: SavedProduct[] | null;
		savedError?: string;
		onRetry?: () => void;
		onRemove: () => void;
		onToggle: () => void;
		onPick: () => void;
		onSwap: (p: Product) => void;
	} = $props();

	const ready = $derived(itemReady(item));
	const suggested = $derived(matchProduct(item, saved));

	const num = (e: Event) => {
		const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
		return Number.isFinite(v) && v >= 0 ? v : null;
	};

	function setGrams(e: Event) {
		item = withGramsInput(item, num(e));
	}

	function setValue(k: NumKey, e: Event) {
		item = k === 'kcal' ? withKcal(item, num(e)) : withValue(item, k, num(e));
	}

	/** A blank or zero amount is an unfinished edit, so the stored value comes back on blur. */
	function restoreAmount(e: FocusEvent, shown: string | number) {
		const input = e.currentTarget as HTMLInputElement;
		const v = input.valueAsNumber;
		if (!Number.isFinite(v) || v <= 0) input.value = String(shown);
	}

	const amountUnit = $derived(item.amount_unit ?? 'g');

	function pickUnit(e: Event) {
		const unit = (e.currentTarget as HTMLSelectElement).value;
		const portion = item.portions?.find((p) => p.unit === unit);
		if (!portion) return;
		const quantity = item.quantity ?? 1;
		item = withQuantity({ ...item, portion }, quantity);
	}

	function setQuantity(e: Event) {
		item = withQuantity(item, num(e));
	}

	function bump(direction: 1 | -1) {
		item = withQuantity(item, stepQuantity(item.quantity ?? 1, direction));
	}
</script>

<li class="item" class:needs={!ready}>
	<div class="item-head">
		{#if item.editable}
			<label class="visually-hidden" for="name-{item.id}">Item name</label>
			<input
				id="name-{item.id}"
				class="name-input"
				placeholder="Food name"
				bind:value={item.name}
				aria-invalid={!item.name.trim()}
			/>
		{:else}
			<h3 class="name">{item.name}</h3>
		{/if}
		<button type="button" class="btn btn-quiet remove" aria-label="Remove {item.name || 'item'}" onclick={onRemove}>
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
		<span class="num row-kcal">{formatKcal(item.kcal ?? 0)} <small>kcal</small></span>
	</p>

	{#if suggested}
		<div class="row suggest">
			<span class="chip chip-db">Saved product</span>
			<button type="button" class="btn btn-quiet" onclick={() => onSwap(suggested)}>
				Use {suggested.name}
			</button>
		</div>
	{/if}

	{#if item.options.length}
		<fieldset class="choice">
			<legend class="small">Which {item.options[0].name.toLowerCase()}?</legend>
			{#each item.options as o, k (k)}
				<label class="row">
					<input
						type="radio"
						name="choice-{item.id}"
						checked={item.chosen === k}
						onchange={() => (item = choose(item, k))}
					/>
					<span>
						<span class="opt">{o.name}</span>
						<span class="muted small">· {o.origin === 'food_db' ? 'Database' : 'AI estimate'}</span>
					</span>
				</label>
			{/each}
		</fieldset>
	{/if}

	<div class="qty">
		{#if item.portions?.length && item.portion}
			<button type="button" class="step" aria-label="Less {item.name || 'item'}" onclick={() => bump(-1)}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 12h12" /></svg>
			</button>
			<label class="visually-hidden" for="quantity-{item.id}">Quantity</label>
			<input
				id="quantity-{item.id}"
				class="qty-input"
				type="number"
				inputmode="decimal"
				min="0"
				step="any"
				value={item.quantity ?? 1}
				oninput={setQuantity}
				onblur={(e) => restoreAmount(e, item.quantity ?? 1)}
			/>
			<button type="button" class="step" aria-label="More {item.name || 'item'}" onclick={() => bump(1)}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 6v12M6 12h12" /></svg>
			</button>
			<label class="visually-hidden" for="unit-{item.id}">Unit</label>
			<select id="unit-{item.id}" class="unit" value={item.portion.unit} onchange={pickUnit}>
				{#each item.portions as p (p.unit)}
					<option value={p.unit}>{p.unit} ({formatGrams(p.grams)} {amountUnit})</option>
				{/each}
			</select>
		{/if}
		<span class="grams">
			<label class="visually-hidden" for="grams-{item.id}">Grams</label>
			<input
				id="grams-{item.id}"
				type="number"
				inputmode="decimal"
				min="0"
				step="any"
				value={item.grams === null ? '' : Number(formatGrams(item.grams))}
				oninput={setGrams}
				onblur={(e) => restoreAmount(e, item.grams === null ? '' : Number(formatGrams(item.grams)))}
			/>
			<span aria-hidden="true">{amountUnit}</span>
		</span>
	</div>
	{#if item.editable || open}
		<div class="nums">
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
						oninput={(e) => setValue(k, e)}
						aria-invalid={item[k] === null}
					/>
				</div>
			{/each}
		</div>
	{:else}
		<p class="values small muted num">
			P {formatGrams(item.protein_g ?? 0)} · C {formatGrams(item.carbs_g ?? 0)} · F {formatGrams(
				item.fat_g ?? 0
			)} g
		</p>
	{/if}

	<div class="row actions">
		{#if !item.editable}
			<button
				type="button"
				class="btn btn-quiet"
				aria-expanded={open}
				aria-controls="kcal-{item.id}"
				onclick={onToggle}>Edit numbers</button
			>
		{/if}
		<button type="button" class="btn" aria-expanded={picking} aria-controls="picker-{item.id}" onclick={onPick}>
			Use my product
		</button>
	</div>

	{#if picking}
		<ReviewProductPicker
			itemId={item.id}
			mode="replace"
			{saved}
			{savedError}
			{onRetry}
			onPick={onSwap}
		/>
	{/if}

	{#if item.swapped && item.phrase.trim()}
		<label class="row">
			<input type="checkbox" bind:checked={item.remember} />
			<span>Always use this for '{item.phrase}'</span>
		</label>
	{/if}
</li>

<style>
	.item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border-top: 1px solid var(--border);
	}
	.item:first-child {
		border-top: 0;
	}
	.item.needs {
		box-shadow: inset 3px 0 0 var(--warning);
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
		align-items: center;
	}
	.row-kcal {
		margin-left: auto;
		font-family: var(--font-display);
		font-size: var(--text-md);
		font-weight: 700;
	}
	.row-kcal small {
		font-family: var(--font-sans);
		font-weight: var(--weight-regular, 400);
		color: var(--text-muted);
		font-size: var(--text-xs);
	}
	.suggest {
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
	/* The amount row: - qty + unit on the left, grams on the right. */
	.qty {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.step {
		display: grid;
		place-items: center;
		flex: none;
		inline-size: var(--touch);
		block-size: var(--touch);
		padding: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
		transition:
			scale var(--dur-fast) var(--ease),
			background var(--dur-fast) var(--ease);
	}
	.step:hover {
		background: var(--surface);
	}
	.step:active {
		scale: 0.94;
	}
	.step svg {
		inline-size: 1.125rem;
		block-size: 1.125rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
	}
	.qty-input {
		inline-size: 4.5rem;
		text-align: center;
		font-weight: 600;
	}
	.unit {
		inline-size: auto;
		flex: 1 1 7rem;
		min-inline-size: 0;
	}
	.grams {
		margin-inline-start: auto;
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--text-muted);
	}
	.grams input {
		inline-size: 5rem;
		text-align: end;
	}
	.nums {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(5.5rem, 1fr));
		gap: var(--space-2);
		align-items: end;
	}
	.values {
		margin: 0;
	}
	.actions {
		gap: var(--space-1);
	}
	@media (prefers-reduced-motion: reduce) {
		.step:active {
			scale: 1;
		}
	}
</style>
