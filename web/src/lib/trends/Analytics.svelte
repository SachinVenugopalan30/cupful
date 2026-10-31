<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Analytics — the two panels under the charts. Macro balance compares the range's average grams
  with today's server-computed targets; meal mix shares the average eaten kcal across the four
  meals. Both bars grow from the left on mount.
-->
<script lang="ts">
	import type { Budget, MealMix, TrendAverages } from '../api';
	import CountUp from './CountUp.svelte';
	import { formatNumber, mealShares } from './logic.ts';

	let {
		averages,
		mealMix,
		targets,
		days
	}: {
		averages: TrendAverages | null;
		mealMix: MealMix | null;
		targets: Budget | null;
		days: number;
	} = $props();

	const macros = $derived(
		averages
			? [
					{ key: 'protein', label: 'Protein', value: averages.protein_g, target: targets?.protein_g },
					{ key: 'carbs', label: 'Carbs', value: averages.carbs_g, target: targets?.carbs_g },
					{ key: 'fat', label: 'Fat', value: averages.fat_g, target: targets?.fat_g }
				]
			: []
	);
	const pct = (value: number, target: number | null | undefined) =>
		target && target > 0 ? Math.min(100, (value / target) * 100) : 0;
	const macroLabel = (row: { label: string; value: number; target?: number }) =>
		`${row.label}: ${formatNumber(row.value)} grams average${
			row.target === undefined ? '' : `, target ${formatNumber(row.target)} grams`
		}`;
	const macroValue = (row: { value: number; target?: number }) =>
		row.target === undefined
			? `${formatNumber(row.value)} g`
			: `${formatNumber(row.value)} g / ${formatNumber(row.target)} g`;

	const shares = $derived(mealShares(mealMix));
	const mixTotal = $derived(shares?.reduce((a, s) => a + s.kcal, 0) ?? 0);
	const mixSummary = $derived(
		shares
			? `Average calories by meal: ${shares.map((s) => `${s.label} ${s.pct}%`).join(', ')}.`
			: ''
	);
</script>

<div class="analytics">
	<section class="card panel" aria-labelledby="macro-h">
		<div class="head">
			<h2 id="macro-h">Macro balance</h2>
			<span class="muted small">{days}-day average</span>
		</div>
		{#if !averages}
			<p class="empty">
				No meals logged in this range. Your macro average appears once you log a meal.
			</p>
		{:else}
			<div class="balance">
				{#each macros as row, i (row.key)}
					<div class="balance-row">
						<div class="balance-head">
							<span>{row.label}</span>
							<span class="num">{macroValue(row)}</span>
						</div>
						<div
							class="balance-track"
							role="img"
							aria-label={macroLabel(row)}
						>
							<i
								class={row.key}
								style:width={`${pct(row.value, row.target)}%`}
								style:--i={i}
							></i>
						</div>
					</div>
				{/each}
			</div>
			{#if !targets}
				<p class="hint">Save your profile for daily targets.</p>
			{/if}
		{/if}
	</section>

	<section class="card panel" aria-labelledby="mix-h">
		<div class="head">
			<h2 id="mix-h">Meal mix</h2>
			{#if shares}
				<span class="muted small"><CountUp value={mixTotal} /> kcal average</span>
			{/if}
		</div>
		{#if !shares}
			<p class="empty">No meals logged in this range. The meal mix appears once you log a meal.</p>
		{:else}
			<div class="mix-bar" role="img" aria-label={mixSummary}>
				{#each shares as share, i (share.key)}
					<i
						class={share.key}
						style:width={`${share.pct}%`}
						style:--i={i}
						aria-hidden="true"
					></i>
				{/each}
			</div>
			<ul class="mix-list">
				{#each shares as share (share.key)}
					<li>
						<i class="swatch {share.key}" aria-hidden="true"></i>
						<span>{share.label}</span>
						<span class="num">{formatNumber(share.kcal)} kcal</span>
						<span class="muted num">{share.pct}%</span>
					</li>
				{/each}
			</ul>
		{/if}
	</section>
</div>

<style>
	.analytics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-4);
	}
	.panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
	}
	h2 {
		margin: 0;
	}
	.balance {
		display: grid;
		gap: var(--space-3);
	}
	.balance-head {
		display: flex;
		justify-content: space-between;
		gap: var(--space-2);
		font-size: var(--text-xs);
		margin-bottom: var(--space-1);
	}
	.balance-track {
		height: 0.5rem;
		border-radius: var(--radius-pill);
		background: var(--surface-2);
		overflow: hidden;
	}
	.balance-track i {
		display: block;
		height: 100%;
		border-radius: var(--radius-pill);
		transform-origin: left;
		animation: grow-x 620ms var(--ease) both;
		animation-delay: calc(var(--i) * 60ms);
	}
	.balance-track i.protein {
		background: var(--protein);
	}
	.balance-track i.carbs {
		background: var(--carbs);
	}
	.balance-track i.fat {
		background: var(--fat);
	}
	.mix-bar {
		display: flex;
		gap: 2px;
		height: 0.875rem;
		border-radius: var(--radius-pill);
		overflow: hidden;
	}
	.mix-bar i {
		display: block;
		height: 100%;
		transform-origin: left;
		animation: grow-x 620ms var(--ease) both;
		animation-delay: calc(var(--i) * 60ms);
	}
	.mix-bar i.breakfast,
	.swatch.breakfast {
		background: var(--breakfast);
	}
	.mix-bar i.lunch,
	.swatch.lunch {
		background: var(--lunch);
	}
	.mix-bar i.dinner,
	.swatch.dinner {
		background: var(--dinner);
	}
	.mix-bar i.snack,
	.swatch.snack {
		background: var(--snack);
	}
	.mix-list {
		list-style: none;
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
	}
	.mix-list li {
		display: grid;
		grid-template-columns: 0.625rem 1fr auto auto;
		gap: var(--space-2);
		align-items: center;
		font-size: var(--text-xs);
	}
	.swatch {
		width: 0.625rem;
		height: 0.625rem;
		border-radius: 0.125rem;
	}
	@keyframes grow-x {
		from {
			transform: scaleX(0);
		}
		to {
			transform: none;
		}
	}
	@media (max-width: 40rem) {
		.analytics {
			grid-template-columns: 1fr;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.balance-track i,
		.mix-bar i {
			animation: none;
		}
	}
</style>
