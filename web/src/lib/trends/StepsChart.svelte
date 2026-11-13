<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  StepsChart — steps per day against your typical day (the profile has no step goal). Bars grow on
  mount and replay when the range changes; tap a bar to read its day.
-->
<script lang="ts">
	import type { TrendDay } from '../api';
	import CountUp from '#lib/motion/CountUp.svelte';
	import ChartCard from './ChartCard.svelte';
	import { axisLabels, formatNumber, stepsReadout, stepsStats } from './logic.ts';

	let { days, today, typicalSteps }: { days: TrendDay[]; today: string; typicalSteps: number | null } = $props();

	const s = $derived(stepsStats(days, typicalSteps));
	const max = $derived(Math.max(1, s.best ?? 0, typicalSteps ?? 0) * 1.05);
	const typicalY = $derived(typicalSteps !== null && typicalSteps > 0 ? 100 - (typicalSteps / max) * 100 : null);
	const labels = $derived(
		axisLabels(
			days.map((d) => d.date),
			today
		)
	);
	const gapNote = $derived.by(() => {
		if (s.avg === null) return '';
		const best = `best ${formatNumber(s.best ?? 0)}`;
		if (typicalSteps === null || typicalSteps <= 0 || s.diff === null) return `Best ${formatNumber(s.best ?? 0)}`;
		const diff = Math.round(s.diff);
		const gap =
			diff === 0
				? `on your ${formatNumber(typicalSteps)} typical day`
				: `${formatNumber(Math.abs(diff))} ${diff < 0 ? 'below' : 'above'} your ${formatNumber(typicalSteps)} typical day`;
		return `${gap} · ${best}`;
	});

	let selected = $state<TrendDay | null>(null);
	$effect(() => {
		void days.length; // a new range clears the tapped day
		selected = null;
	});
</script>

<ChartCard
	title="Steps"
	aside={s.count ? `${s.count} days recorded` : ''}
	empty={s.count ? '' : 'No steps recorded in this range. Add them under Activity, or set up the Shortcut in Settings.'}
	readout={selected ? stepsReadout(selected, today) : ''}
	summary="Steps each day, {days.length} days. {s.count} days recorded, average {Math.round(s.avg ?? 0)} steps, best {s.best ?? 0}."
>
	{#snippet figure()}
		{#if s.avg !== null}<CountUp value={s.avg} /> <small>a day</small>{/if}
	{/snippet}
	{#snippet note()}{gapNote}{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch" style="--c: var(--steps)"></i>Steps</span>
		{#if typicalY !== null}<span class="key"><i class="swatch tick" style="--c: var(--text)"></i>Typical day</span>{/if}
	{/snippet}
	<div class="chart-wrap">
		{#key days.length}
			<svg class="chart" viewBox="0 0 {days.length} 100" preserveAspectRatio="none" aria-hidden="true">
				{#each days as d, i (d.date)}
					{#if d.steps !== null}
						<!-- Pointer readout only; the card's summary is the keyboard path (design.md). -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<rect
							role="presentation"
							class="bar grow-y"
							x={i + 0.12}
							y={100 - (d.steps / max) * 100}
							width={0.76}
							height={(d.steps / max) * 100}
							style:--n={Math.min(i, 24) * 0.5}
							onpointerenter={() => (selected = d)}
							onpointerdown={() => (selected = d)}
							onclick={() => (selected = d)}
						/>
					{/if}
				{/each}
				{#if typicalY !== null}
					<line class="typical grow-x" style="--n: 6" x1="0" x2={days.length} y1={typicalY} y2={typicalY} vector-effect="non-scaling-stroke" />
				{/if}
			</svg>
		{/key}
		<div class="axis" aria-hidden="true">
			{#each labels as l}<span>{l}</span>{/each}
		</div>
	</div>
</ChartCard>

<style>
	.bar {
		fill: var(--steps);
		opacity: 0.85;
	}
	.typical {
		stroke: var(--text);
		stroke-width: 1.5;
		stroke-dasharray: 4 3;
		opacity: 0.6;
	}
</style>
