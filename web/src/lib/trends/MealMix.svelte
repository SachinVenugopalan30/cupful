<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  MealMix — the share of the range's average eaten kcal by meal, as one bar that grows from the
  left and a legend with the kcal and percent. The bar replays when the range changes.
  Props: mealMix: MealMix | null (server-side averages).
-->
<script lang="ts">
	import type { MealMix } from '../api';
	import CountUp from '#lib/motion/CountUp.svelte';
	import { formatNumber, mealShares } from './logic.ts';

	let { mealMix, days }: { mealMix: MealMix | null; days: number } = $props();

	const shares = $derived(mealShares(mealMix));
	const total = $derived(shares?.reduce((a, s) => a + s.kcal, 0) ?? 0);
	const summary = $derived(
		shares ? `Average calories by meal: ${shares.map((s) => `${s.label} ${s.pct}%`).join(', ')}.` : ''
	);
</script>

<section class="card mix" aria-labelledby="mix-h">
	<div class="card-head">
		<h2 id="mix-h">Meal mix</h2>
		{#if shares}<span><CountUp value={total} /> kcal average</span>{:else}<span>share of kcal</span>{/if}
	</div>
	{#if !shares}
		<p class="empty">No meals logged in this range. The meal mix appears once you log a meal.</p>
	{:else}
		{#key days}
			<div class="bar" role="img" aria-label={summary}>
				{#each shares as share, i (share.key)}
					<i class="{share.key} grow-x" style:flex={share.pct} style="--n: {i * 2}" aria-hidden="true"></i>
				{/each}
			</div>
		{/key}
		<ul class="legend">
			{#each shares as share (share.key)}
				<li>
					<i class="swatch {share.key}" aria-hidden="true"></i>
					<span>{share.label}</span>
					<span class="num kcal">{formatNumber(share.kcal)} kcal</span>
					<span class="num muted">{share.pct}%</span>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.mix {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.bar {
		display: flex;
		gap: 2px;
		height: 1rem;
		border-radius: var(--radius-pill);
		overflow: hidden;
	}
	.bar i {
		display: block;
		height: 100%;
		min-width: 0;
	}
	.breakfast {
		background: var(--breakfast);
	}
	.lunch {
		background: var(--lunch);
	}
	.dinner {
		background: var(--dinner);
	}
	.snack {
		background: var(--snack);
	}
	.legend {
		list-style: none;
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
	}
	.legend li {
		display: grid;
		grid-template-columns: 0.75rem 1fr auto 2.5rem;
		gap: var(--space-2);
		align-items: center;
		font-size: var(--text-sm);
	}
	.legend .muted {
		text-align: right;
	}
	.kcal {
		color: var(--text-muted);
	}
	.swatch {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 3px;
	}
</style>
