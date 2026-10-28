<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- "Enter from the label": a product the database doesn't have, saved for everyone on this Cupful. -->
<script lang="ts">
	import { errorMessage, products, type Product } from '../api';
	import { labelProblem, type LabelForm } from './logic';

	let { barcode, onSaved }: { barcode: string; onSaved: (p: Product) => void } = $props();

	// Number inputs bind to null when empty.
	let f = $state({
		name: '',
		brand: '',
		// svelte-ignore state_referenced_locally
		barcode,
		basis: 'serving' as LabelForm['basis'],
		serving_g: null as number | null,
		kcal: null as number | null,
		protein_g: null as number | null,
		carbs_g: null as number | null,
		fat_g: null as number | null
	});
	let problem = $state<ReturnType<typeof labelProblem>>(null);
	let error = $state('');
	let busy = $state(false);

	const unit = $derived(f.basis === 'serving' ? 'per serving' : 'per 100 g');
	const num = (v: number | null) => (v === null || v === undefined ? NaN : Number(v));
	const invalid = (field: keyof LabelForm) => (problem?.field === field ? 'true' : undefined);
	const describedBy = (field: keyof LabelForm) => (problem?.field === field ? 'label-problem' : undefined);

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const form: LabelForm = {
			...f,
			serving_g: f.serving_g === null || f.serving_g === undefined ? null : Number(f.serving_g),
			kcal: num(f.kcal),
			protein_g: num(f.protein_g),
			carbs_g: num(f.carbs_g),
			fat_g: num(f.fat_g)
		};
		problem = labelProblem(form);
		if (problem) {
			document.getElementById(`label-${problem.field}`)?.focus();
			return;
		}
		busy = true;
		try {
			onSaved(
				await products.label({
					...form,
					name: form.name.trim(),
					brand: form.brand.trim() || null
				})
			);
		} catch (e) {
			error = errorMessage(e, 'product');
		} finally {
			busy = false;
		}
	}
</script>

<form class="stack" onsubmit={save} novalidate>
	<p class="notice notice-info">This product isn't in the database yet. Enter it from the label.</p>

	<div class="field">
		<label for="label-name">Name</label>
		<input id="label-name" bind:value={f.name} maxlength="200" aria-invalid={invalid('name')} aria-describedby={describedBy('name')} />
	</div>
	<div class="field">
		<label for="label-brand">Brand <span class="muted">(optional)</span></label>
		<input id="label-brand" bind:value={f.brand} maxlength="200" aria-invalid={invalid('brand')} aria-describedby={describedBy('brand')} />
	</div>
	<div class="field">
		<label for="label-barcode">Barcode</label>
		<input id="label-barcode" inputmode="numeric" autocomplete="off" bind:value={f.barcode} aria-invalid={invalid('barcode')} aria-describedby={describedBy('barcode')} />
	</div>

	<fieldset class="segmented">
		<legend class="visually-hidden">The label gives values</legend>
		<label><input type="radio" name="basis" value="serving" bind:group={f.basis} /> Per serving</label>
		<label><input type="radio" name="basis" value="100g" bind:group={f.basis} /> Per 100 g</label>
	</fieldset>

	<div class="field">
		<label for="label-serving_g">Serving size (g){#if f.basis === '100g'} <span class="muted">(optional)</span>{/if}</label>
		<input id="label-serving_g" type="number" inputmode="decimal" min="0" step="any" bind:value={f.serving_g} aria-invalid={invalid('serving_g')} aria-describedby={describedBy('serving_g')} />
		{#if f.basis === '100g'}<p class="hint">Lets you log by servings.</p>{/if}
	</div>

	<div class="grid">
		{#each [['kcal', 'Calories (kcal)'], ['protein_g', 'Protein (g)'], ['carbs_g', 'Carbs (g)'], ['fat_g', 'Fat (g)']] as const as [key, text] (key)}
			<div class="field">
				<label for="label-{key}">{text} <span class="visually-hidden">{unit}</span></label>
				<input id="label-{key}" type="number" inputmode="decimal" min="0" step="any" bind:value={f[key]} aria-invalid={invalid(key)} aria-describedby={describedBy(key)} />
			</div>
		{/each}
	</div>
	<p class="hint">Values {unit}, as printed on the label.</p>

	{#if problem}<p id="label-problem" class="notice notice-error" role="alert">{problem.message}</p>{/if}
	{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}

	<button class="btn btn-primary btn-block" disabled={busy}>Save product</button>
</form>

<style>
	fieldset {
		min-width: 0;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-3);
	}
</style>
