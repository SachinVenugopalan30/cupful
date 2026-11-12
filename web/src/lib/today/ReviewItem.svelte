<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- One item row in Review: name, origin, number inputs, household unit picker and the product swap. -->
<script lang="ts">
	import { formatGrams, formatKcal, round } from '#lib/format.ts';
	import { portionGrams, portionLabel } from '#lib/products/portions.ts';
	import type { Product } from '#lib/api.ts';
	import ReviewProductPicker from './ReviewProductPicker.svelte';
	import { MACROS } from './review';
	import {
		choose,
		itemReady,
		withGrams,
		withKcal,
		withValue,
		type DraftItem,
		type NumKey
	} from './meal';

	let {
		item = $bindable(),
		open,
		picking,
		onRemove,
		onToggle,
		onPick,
		onSwap
	}: {
		item: DraftItem;
		open: boolean;
		picking: boolean;
		onRemove: () => void;
		onToggle: () => void;
		onPick: () => void;
		onSwap: (p: Product) => void;
	} = $props();

	const ready = $derived(itemReady(item));

	const num = (e: Event) => {
		const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
		return Number.isFinite(v) && v >= 0 ? v : null;
	};

	/** Grams and the portion count follow each other; macros scale with the grams. */
	function setGrams(e: Event) {
		const g = num(e);
		if (g === null || g <= 0) {
			if (item.editable) item = { ...item, grams: null };
			return;
		}
		const next = withGrams(item, g);
		if (!next.portion) return void (item = next);
		const quantity = round(g / next.portion.grams, 2);
		item = { ...next, quantity, portion_text: portionLabel(quantity, next.portion) };
	}

	function setValue(k: NumKey, e: Event) {
		item = k === 'kcal' ? withKcal(item, num(e)) : withValue(item, k, num(e));
	}

	function pickUnit(e: Event) {
		const unit = (e.currentTarget as HTMLSelectElement).value;
		const portion = item.portions?.find((p) => p.unit === unit);
		if (!portion) return;
		const quantity = item.quantity ?? 1;
		const next = withGrams({ ...item, portion }, portionGrams(quantity, portion));
		item = { ...next, portion_text: portionLabel(quantity, portion) };
	}

	function setQuantity(e: Event) {
		const portion = item.portion;
		if (!portion) return;
		const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
		const quantity = Number.isFinite(v) && v >= 0 ? v : 0;
		const next = withGrams(item, portionGrams(quantity, portion));
		item = { ...next, quantity, portion_text: portionLabel(quantity, portion) };
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

	<div class="nums">
		{#if item.portions?.length && item.portion}
			<div class="field">
				<label for="unit-{item.id}">Unit</label>
				<select id="unit-{item.id}" value={item.portion.unit} onchange={pickUnit}>
					{#each item.portions as p (p.unit)}
						<option value={p.unit}>{p.unit} ({formatGrams(p.grams)} g)</option>
					{/each}
				</select>
			</div>
			<div class="field">
				<label for="quantity-{item.id}">Quantity</label>
				<input
					id="quantity-{item.id}"
					type="number"
					inputmode="decimal"
					min="0"
					step="any"
					value={item.quantity ?? 1}
					oninput={setQuantity}
				/>
			</div>
		{/if}
		<div class="field">
			<label for="grams-{item.id}">Grams</label>
			<input
				id="grams-{item.id}"
				type="number"
				inputmode="decimal"
				min="0"
				step="any"
				value={item.grams === null ? '' : Number(formatGrams(item.grams))}
				oninput={setGrams}
			/>
		</div>
		{#if item.editable || open}
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
		<ReviewProductPicker itemId={item.id} mode="replace" onPick={onSwap} />
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
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
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
		grid-column: 1 / -1;
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
</style>
