<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The product sheet: values, servings (0.5 steps) or grams, meal type, Add. -->
<script lang="ts">
	import { errorMessage, type MealType, type Product } from '../api';
	import { formatGrams, formatKcal } from '../format';
	import { mealUpsert, sendOps } from '../sync';
	import { buildMeal, defaultMealType, gramsToServings, servingsToGrams, stepServings } from './logic';

	let {
		product: p,
		servings,
		date,
		onAdded
	}: { product: Product; servings: number | null; date: string; onAdded: () => void } = $props();

	// svelte-ignore state_referenced_locally
	const sg = p.serving_g;
	// svelte-ignore state_referenced_locally
	let grams = $state<number | null>(sg ? servingsToGrams(servings ?? 1, sg) : 100);
	let mealType = $state<MealType>(defaultMealType(new Date().getHours()));
	let busy = $state(false);
	let error = $state('');

	const ok = $derived(grams !== null && grams > 0 && grams <= 10_000);
	const amount = $derived(ok ? Number(grams) : 0);
	const count = $derived(sg && ok ? gramsToServings(amount, sg) : 0);
	const at = (per100: number, g: number) => (per100 * g) / 100;

	const rows = [
		['Calories', 'kcal', ''],
		['Protein', 'protein_g', ' g'],
		['Carbs', 'carbs_g', ' g'],
		['Fat', 'fat_g', ' g']
	] as const;
	const show = (v: number, key: string) => (key === 'kcal' ? formatKcal(v) : formatGrams(v));

	function step(dir: 1 | -1) {
		if (sg) grams = servingsToGrams(stepServings(count || 1, dir), sg);
	}

	async function add() {
		if (!ok) return;
		busy = true;
		error = '';
		try {
			const [r] = await sendOps([mealUpsert(buildMeal(p, amount, date, mealType))]);
			if (r.status === 'applied' || r.status === 'duplicate') return onAdded();
			error = r.error ? `Not saved: ${r.error}` : `Not saved (${r.status}).`;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="stack">
	{#if p.brand}<p class="muted">{p.brand}</p>{/if}

	<table class="values small">
		<caption class="visually-hidden">Nutrition values</caption>
		<thead>
			<tr>
				<td></td>
				{#if sg}<th scope="col">Per serving<br /><span class="muted">{p.serving_text ?? `${formatGrams(sg)} g`}</span></th>{/if}
				<th scope="col">Per 100 g</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as [label, key, u] (key)}
				<tr>
					<th scope="row">{label}</th>
					{#if sg}<td class="num">{show(at(p[key], sg), key)}{u}</td>{/if}
					<td class="num">{show(p[key], key)}{u}</td>
				</tr>
			{/each}
		</tbody>
	</table>

	<div class="amount">
		{#if sg}
			<div class="field">
				<span class="label" id="servings-label">Servings</span>
				<div class="stepper" role="group" aria-labelledby="servings-label">
					<button type="button" class="btn" aria-label="Fewer servings" disabled={count <= 0.5} onclick={() => step(-1)}>−</button>
					<output class="num" aria-live="polite">{count}</output>
					<button type="button" class="btn" aria-label="More servings" onclick={() => step(1)}>+</button>
				</div>
			</div>
		{/if}
		<div class="field">
			<label for="product-grams">Grams</label>
			<input
				id="product-grams"
				type="number"
				inputmode="decimal"
				min="0"
				max="10000"
				step="any"
				bind:value={grams}
				aria-invalid={ok ? undefined : 'true'}
			/>
		</div>
	</div>

	<p class="total" aria-live="polite">
		<span class="num kcal">{formatKcal(at(p.kcal, amount))}</span> kcal
		<span class="muted small">
			· P <span class="num">{formatGrams(at(p.protein_g, amount))}</span> g
			· C <span class="num">{formatGrams(at(p.carbs_g, amount))}</span> g
			· F <span class="num">{formatGrams(at(p.fat_g, amount))}</span> g
		</span>
	</p>

	<fieldset class="segmented">
		<legend class="visually-hidden">Meal</legend>
		{#each ['breakfast', 'lunch', 'dinner', 'snack'] as const as t (t)}
			<label><input type="radio" name="product-meal" value={t} bind:group={mealType} /> {t[0].toUpperCase() + t.slice(1)}</label>
		{/each}
	</fieldset>

	{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}

	<button type="button" class="btn btn-primary btn-block" disabled={!ok || busy} onclick={add}>Add</button>

	{#if p.attribution}
		<p class="small muted">
			<a href={p.attribution.url} target="_blank" rel="noopener noreferrer">{p.attribution.text}</a>
		</p>
	{/if}
</div>

<style>
	.values {
		width: 100%;
		border-collapse: collapse;
	}
	.values th,
	.values td {
		padding: var(--space-1) 0;
		text-align: right;
		font-weight: var(--weight-regular);
	}
	.values thead th {
		font-weight: var(--weight-medium);
		vertical-align: bottom;
	}
	.values tbody th {
		text-align: left;
	}
	.values tbody tr + tr {
		border-top: 1px solid var(--border);
	}
	.amount {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
		gap: var(--space-3);
	}
	.stepper {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.stepper output {
		flex: 1;
		text-align: center;
		font-size: var(--text-lg);
	}
	.stepper .btn {
		font-size: var(--text-lg);
		padding: 0;
	}
	.total {
		margin: 0;
	}
	.kcal {
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
	}
	fieldset {
		min-width: 0;
	}
</style>
