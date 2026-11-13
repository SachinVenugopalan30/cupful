<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  IntakeChart — the average a day and one bar per day against the budget line. Bars over that day's
  budget use --over. The average counts from its old value to the new one when the range changes;
  the bars and the line replay their entrance (the plot is keyed on the number of days, the figure
  is not). Tapping a bar reads that day out in the aria-live line.
-->
<script lang="ts">
	import type { TrendDay } from '../api';
	import CountUp from '#lib/motion/CountUp.svelte';
	import ChartCard from './ChartCard.svelte';
	import { averageNote, averages, axisLabels, intakeReadout } from './logic.ts';

	let { days, today }: { days: TrendDay[]; today: string } = $props();

	const avg = $derived(averages(days));
	const logged = $derived(days.filter((d) => d.eaten_kcal !== null));
	const isOver = (d: TrendDay) => d.budget_kcal !== null && d.eaten_kcal! > d.budget_kcal;
	const overDays = $derived(logged.filter(isOver).length);
	const max = $derived(Math.max(1, ...days.flatMap((d) => [d.eaten_kcal ?? 0, d.budget_kcal ?? 0])) * 1.05);
	const labels = $derived(
		axisLabels(
			days.map((d) => d.date),
			today
		)
	);

	/** The budget line as runs of points; a day with no budget breaks it, never drops it to zero. */
	const runs = $derived.by(() => {
		const out: string[] = [];
		let run: string[] = [];
		days.forEach((d, i) => {
			if (d.budget_kcal === null) {
				if (run.length) out.push(run.join(' '));
				run = [];
			} else run.push(`${i + 0.5},${100 - (d.budget_kcal / max) * 100}`);
		});
		if (run.length) out.push(run.join(' '));
		return out;
	});

	let selected = $state<TrendDay | null>(null);
	$effect(() => {
		void days.length; // a new range clears the tapped day
		selected = null;
	});
	const summary = $derived(
		`Calories eaten each day, ${days.length} days, against the daily budget. ${logged.length} days with meals, average ${Math.round(avg.intake ?? 0)} kcal against an average budget of ${Math.round(avg.budget ?? 0)} kcal; over budget on ${overDays} of them.`
	);
</script>

<ChartCard
	title="Average a day"
	label="Average intake against budget"
	aside={logged.length ? `${logged.length} days logged` : ''}
	big
	empty={logged.length ? '' : 'No meals logged in this range. Log a meal on Today and its day shows up here.'}
	readout={selected ? intakeReadout(selected, today) : ''}
	{summary}
>
	{#snippet figure()}
		{#if avg.intake !== null}<CountUp value={avg.intake} /> <small>kcal eaten</small>{/if}
	{/snippet}
	{#snippet note()}{averageNote(avg)}{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch" style="--c: var(--accent)"></i>Intake</span>
		<span class="key"><i class="swatch" style="--c: var(--over)"></i>Over budget</span>
		<span class="key"><i class="swatch tick" style="--c: var(--text)"></i>Budget</span>
	{/snippet}
	<div class="chart-wrap">
		{#key days.length}
			<svg class="chart" viewBox="0 0 {days.length} 100" preserveAspectRatio="none" aria-hidden="true">
				{#each days as d, i (d.date)}
					{#if d.eaten_kcal !== null}
						<!-- Pointer readout only; the card's summary is the keyboard path (design.md). -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<rect
							role="presentation"
							class="bar grow-y"
							class:over={isOver(d)}
							x={i + 0.12}
							y={100 - (d.eaten_kcal / max) * 100}
							width={0.76}
							height={(d.eaten_kcal / max) * 100}
							style:--n={Math.min(i, 24) * 0.5}
							onpointerenter={() => (selected = d)}
							onpointerdown={() => (selected = d)}
							onclick={() => (selected = d)}
						/>
					{/if}
				{/each}
				<g class="target grow-x" style="--n: 6">
					{#each runs as points}<polyline {points} vector-effect="non-scaling-stroke" />{/each}
				</g>
			</svg>
		{/key}
		<div class="axis" aria-hidden="true">
			{#each labels as l}<span>{l}</span>{/each}
		</div>
	</div>
</ChartCard>

<style>
	.bar {
		fill: var(--accent);
		opacity: 0.85;
	}
	.bar.over {
		fill: var(--over);
		opacity: 1;
	}
	.target polyline {
		fill: none;
		stroke: var(--text);
		stroke-width: 2;
		stroke-linejoin: round;
		opacity: 0.85;
	}
</style>
