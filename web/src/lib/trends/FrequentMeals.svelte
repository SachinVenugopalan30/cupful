<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  FrequentMeals — "Eaten most" for the selected Trends range. Display only.
  Props: meals: TopMeal[]; days: number.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { TopMeal } from '../api';
	import { formatNumber, shortDate } from './logic.ts';

	let { meals, days }: { meals: TopMeal[]; days: number } = $props();
	const top = $derived(Math.max(1, ...meals.map((m) => m.times)));
	// Only the rows present when the range mounted animate. A data refresh may add a row;
	// that row should not replay the entrance.
	const initial = new Set(untrack(() => meals.map((m) => m.label)));
	const type = (t: string) => t[0].toUpperCase() + t.slice(1);
	const times = (n: number) => (n === 1 ? '1 time' : `${n} times`);
</script>

<section class="card frequent" aria-labelledby="frequent-h">
	<div class="head">
		<h2 id="frequent-h">Eaten most</h2>
		<span class="muted small">{days}-day count</span>
	</div>
	{#if !meals.length}
		<p class="empty">No meals logged in this range.</p>
	{:else}
		<ul class="list">
			{#each meals as m, i (m.label)}
				<li
					aria-label="{m.label}, {times(m.times)}, {formatNumber(m.avg_kcal)} kcal average, last {shortDate(m.last_date)}"
				>
					<span class="label">{m.label}</span>
					<span class="meta small muted">
						{type(m.meal_type)} · {times(m.times)} · last {shortDate(m.last_date)}
					</span>
					<span class="num">{formatNumber(m.avg_kcal)} kcal avg</span>
					<i
						class="bar"
						class:animate={initial.has(m.label)}
						style:width={`${(m.times / top) * 100}%`}
						style:--i={i}
					></i>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.frequent {
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
	.list {
		list-style: none;
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
	}
	li {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0 var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}
	li:last-child {
		border-bottom: 0;
	}
	.label {
		font-weight: var(--weight-medium);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		grid-column: 1;
	}
	.num {
		grid-row: 1 / span 2;
		grid-column: 2;
		align-self: center;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	.bar {
		position: absolute;
		left: 0;
		bottom: 0;
		height: 2px;
		background: var(--accent);
		border-radius: var(--radius-pill);
		transform-origin: left;
	}
	.bar.animate {
		animation: grow-x 760ms var(--ease) both;
		animation-delay: calc(var(--i) * 80ms);
	}
	@keyframes grow-x {
		from {
			transform: scaleX(0);
		}
		to {
			transform: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bar.animate {
			animation: none;
		}
	}
</style>
