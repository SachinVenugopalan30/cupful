<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  MacroBalance — the range's average grams a day against today's server-computed targets. Each bar
  grows from the left on mount and when the range changes.
  Props: averages: TrendAverages | null, targets: Budget | null (today's budget), days: number.
-->
<script lang="ts">
	import type { Budget, TrendAverages } from '../api';
	import { formatNumber } from './logic.ts';

	let { averages, targets, days }: { averages: TrendAverages | null; targets: Budget | null; days: number } = $props();

	const macros = $derived(
		averages
			? [
					{ key: 'protein', label: 'Protein', value: averages.protein_g, target: targets?.protein_g },
					{ key: 'carbs', label: 'Carbs', value: averages.carbs_g, target: targets?.carbs_g },
					{ key: 'fat', label: 'Fat', value: averages.fat_g, target: targets?.fat_g }
				]
			: []
	);
	const share = (value: number, target: number | null | undefined) =>
		target && target > 0 ? Math.min(1, value / target) : 0;
	const name = (row: { label: string; value: number; target?: number }) =>
		`${row.label}: ${formatNumber(row.value)} grams average${
			row.target === undefined ? '' : `, target ${formatNumber(row.target)} grams`
		}`;
	const text = (row: { value: number; target?: number }) =>
		row.target === undefined ? `${formatNumber(row.value)} g` : `${formatNumber(row.value)} / ${formatNumber(row.target)} g`;
</script>

<section class="card macros" aria-labelledby="macro-h">
	<div class="card-head">
		<h2 id="macro-h">Macro balance</h2>
		<span>{days}-day average</span>
	</div>
	{#if !averages}
		<p class="empty">No meals logged in this range. Your macro average appears once you log a meal.</p>
	{:else}
		{#key days}
			<div class="balance">
				{#each macros as row, i (row.key)}
					<div class="row-m">
						<div class="top">
							<span>{row.label}</span>
							<span class="num">{text(row)}</span>
						</div>
						<div class="track" role="img" aria-label={name(row)}>
							<i class="{row.key} grow-x" style="--p: {share(row.value, row.target)}; --n: {i * 2}"></i>
						</div>
					</div>
				{/each}
			</div>
		{/key}
		{#if !targets}<p class="hint">Save your profile for daily targets.</p>{/if}
	{/if}
</section>

<style>
	.macros {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.balance {
		display: grid;
		gap: var(--space-4);
	}
	.top {
		display: flex;
		justify-content: space-between;
		gap: var(--space-2);
		margin-bottom: var(--space-1);
		font-size: var(--text-sm);
	}
	.top .num {
		font-weight: var(--weight-bold);
	}
	.track {
		height: 0.5rem;
		border-radius: var(--radius-pill);
		background: var(--surface-2);
		overflow: hidden;
	}
	.track i {
		display: block;
		height: 100%;
		border-radius: var(--radius-pill);
		transform: scaleX(var(--p));
		transform-origin: left;
	}
	.protein {
		background: var(--protein);
	}
	.carbs {
		background: var(--carbs);
	}
	.fat {
		background: var(--fat);
	}
</style>
