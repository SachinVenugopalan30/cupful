<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  TodayWeek — "This week": kcal eaten each of the last seven days against the plan line, from
  GET /api/trends. Today's bar follows the jug. Desktop only: phones have the Trends tab, so the
  request is skipped below 60rem. Hidden until there is a plan to compare with.
  Props:
    date: string         the day shown on Today; its eaten figure feeds today's bar when it is today
    refreshKey: number   bumped after a write; refetch when it changes
    plan: number | null  the shown day's budget, the fallback for the plan line
    eaten: number        kcal eaten on the shown day
-->
<script lang="ts">
	import { getTrends, type TrendDay } from '#lib/api.ts';
	import { formatKcal } from '#lib/format.ts';
	import { today } from '#lib/session.svelte.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import { barText, weekBars, weekRange } from './week.ts';

	let {
		date,
		refreshKey,
		plan,
		eaten
	}: { date: string; refreshKey: number; plan: number | null; eaten: number } = $props();

	let wide = $state(false);
	let rows = $state<TrendDay[]>([]);

	$effect(() => {
		const q = matchMedia('(min-width: 60rem)');
		const sync = () => (wide = q.matches);
		sync();
		q.addEventListener('change', sync);
		return () => q.removeEventListener('change', sync);
	});

	$effect(() => {
		if (!wide) return;
		void refreshKey;
		void dataVersion.n;
		const { from, to } = weekRange(today());
		let stale = false;
		getTrends(from, to).then(
			(t) => !stale && (rows = t.days),
			() => !stale && (rows = [])
		);
		return () => (stale = true);
	});

	const now = $derived(today());
	const todayRow = $derived(rows.find((r) => r.date === now));
	const planKcal = $derived(todayRow?.budget_kcal ?? plan ?? 0);
	const bars = $derived(
		weekBars(rows, now, date === now ? eaten : (todayRow?.eaten_kcal ?? 0), planKcal)
	);
</script>

{#if wide && planKcal > 0}
	<section class="card week" aria-labelledby="week-h">
		<header class="card-head">
			<h2 id="week-h">This week</h2>
			<span>kcal against the plan</span>
		</header>
		<ol class="bars" aria-label="kcal eaten each day this week">
			<li class="plan-line" aria-hidden="true" style="--at: {bars[0].planAt}">
				<span>{formatKcal(planKcal)}</span>
			</li>
			{#each bars as b, i (b.date)}
				<li class="bar" class:today={b.today} class:over={b.over} title={barText(b)}>
					<span class="visually-hidden">{barText(b)}</span>
					<span class="track" aria-hidden="true">
						<i class="grow-y" style="--h: {b.height}; --n: {i}"></i>
					</span>
					<span class="day" aria-hidden="true">{b.label}</span>
				</li>
			{/each}
		</ol>
	</section>
{/if}

<style>
	.week {
		display: grid;
		gap: var(--space-4);
	}
	.bars {
		--track-h: 5.625rem;
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(7, 1fr);
		gap: var(--space-2);
		align-items: end;
		position: relative;
	}
	.bar {
		display: grid;
		justify-items: center;
		gap: var(--space-1);
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	.track {
		display: flex;
		align-items: end;
		inline-size: 100%;
		max-inline-size: 1.875rem;
		block-size: var(--track-h);
	}
	.track i {
		inline-size: 100%;
		block-size: 100%;
		border-radius: 6px 6px 2px 2px;
		background: var(--accent);
		opacity: 0.55;
		transform-origin: bottom;
		transform: scaleY(var(--h));
		transition:
			transform var(--dur-slow) var(--ease),
			opacity var(--dur-med) var(--ease);
	}
	.over .track i {
		background: var(--over);
	}
	.today .track i {
		opacity: 1;
	}
	.today {
		color: var(--text);
		font-weight: var(--weight-bold);
	}
	/* The plan: one rule across the chart, labelled at its end. */
	.plan-line {
		position: absolute;
		inset-inline: 0;
		inset-block-end: calc(1.25rem + var(--at) * var(--track-h) + var(--space-1));
		border-block-start: 2px solid var(--text);
		opacity: 0.7;
		z-index: 1;
		pointer-events: none;
	}
	.plan-line span {
		position: absolute;
		inset-inline-start: 0;
		inset-block-end: 2px;
		padding: 0 4px;
		border-radius: 4px;
		background: var(--surface);
		font-size: 0.6875rem;
		font-weight: 700;
		color: var(--text);
	}
</style>
