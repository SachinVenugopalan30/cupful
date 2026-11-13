<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  FrequentMeals — "Eaten most" for the selected Trends range. Display only.
  Props: meals: TopMeal[]; days: number.
-->
<script lang="ts">
	import type { TopMeal } from '../api';
	import { formatNumber, shortDate } from './logic.ts';

	let { meals, days }: { meals: TopMeal[]; days: number } = $props();
	const mealTypeLabel = (t: string) => t[0].toUpperCase() + t.slice(1);
	const timesLabel = (n: number) => (n === 1 ? '1 time' : `${n} times`);
</script>

<section class="card frequent" aria-labelledby="frequent-h">
	<div class="card-head">
		<h2 id="frequent-h">Eaten most</h2>
		<span>times in {days} days</span>
	</div>
	{#if !meals.length}
		<p class="empty">No meals logged in this range.</p>
	{:else}
		{#key days}
			<ul class="rows">
				{#each meals as m, i (m.label)}
					<li class="rise" style="--n: {i}">
						<span class="grow">
							<span class="label">{m.label}</span>
							<small>{mealTypeLabel(m.meal_type)} · {timesLabel(m.times)} · last {shortDate(m.last_date)}</small>
						</span>
						<span class="val num">{formatNumber(m.avg_kcal)} kcal avg</span>
					</li>
				{/each}
			</ul>
		{/key}
	{/if}
</section>

<style>
	.frequent {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
	}
	.label {
		font-weight: var(--weight-medium);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.val {
		flex: none;
		font-size: var(--text-sm);
		color: var(--text-muted);
		font-weight: var(--weight-regular);
	}
</style>
