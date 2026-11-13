<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  WeightChart — weigh-ins as a line over a soft area for the range, the change in the headline and
  the distance to the goal. The plot is keyed on the number of days so it replays on a range change;
  the line never uses dash offsets, so it stays continuous. Tap or drag to read a day.
-->
<script lang="ts">
	import type { TrendDay } from '../api';
	import { formatWeight, type WeightUnit } from '../format';
	import CountUp from '#lib/motion/CountUp.svelte';
	import ChartCard from './ChartCard.svelte';
	import { axisLabels, goalRemaining, rollingAverage, signedWeightDelta, weightReadout, weightStats } from './logic.ts';

	let {
		days,
		today,
		unit,
		goalKg
	}: { days: TrendDay[]; today: string; unit: WeightUnit; goalKg: number | null } = $props();

	const w = $derived(weightStats(days));
	const rolling = $derived(rollingAverage(days.map((d) => d.weight_kg)));
	const weighed = $derived(days.map((d, i) => ({ d, i })).filter((x) => x.d.weight_kg !== null));
	const range = $derived.by(() => {
		const values = weighed.map((x) => x.d.weight_kg!);
		if (!values.length) return { lo: 0, hi: 1 };
		const lo = Math.min(...values);
		const hi = Math.max(...values);
		const pad = Math.max(0.3, (hi - lo) * 0.15);
		return { lo: lo - pad, hi: hi + pad };
	});
	const x = (i: number) => (days.length > 1 ? (i / (days.length - 1)) * 100 : 50);
	const y = (kg: number) => 100 - ((kg - range.lo) / (range.hi - range.lo)) * 100;
	const points = $derived(weighed.map((p) => ({ x: x(p.i), y: y(p.d.weight_kg!) })));
	const linePath = $derived(points.map((p, i) => `${i ? 'L' : 'M'}${p.x},${p.y}`).join(' '));
	const areaPath = $derived(
		points.length ? `M${points[0].x},100 ${points.map((p) => `L${p.x},${p.y}`).join(' ')} L${points.at(-1)!.x},100 Z` : ''
	);
	const labels = $derived(
		axisLabels(
			days.map((d) => d.date),
			today
		)
	);
	const goal = $derived(goalRemaining(w.latest, goalKg, unit));
	const monthWord = $derived(days.length >= 30 ? 'this month' : 'this range');
	const display = (kg: number) => (unit === 'kg' ? kg : kg / 0.45359237);

	let selected = $state<{ d: TrendDay; i: number } | null>(null);
	$effect(() => {
		void days.length; // a new range clears the tapped day
		selected = null;
	});
	let svg: SVGSVGElement | undefined = $state();
	function pick(e: PointerEvent) {
		if (!svg || !weighed.length) return;
		const box = svg.getBoundingClientRect();
		const target = Math.round(Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) * (days.length - 1));
		selected = weighed.reduce((best, p) => (Math.abs(p.i - target) < Math.abs(best.i - target) ? p : best));
	}
</script>

<ChartCard
	title="Weight"
	aside={w.count ? `${w.count} weigh-ins` : ''}
	empty={weighed.length ? '' : 'No weigh-ins in this range. Add one under Activity and the trend starts here.'}
	readout={selected ? weightReadout(selected.d, rolling[selected.i], today, unit) : ''}
	summary="Weigh-ins {days.length} days: {w.count} in all, first {w.first === null
		? '—'
		: formatWeight(w.first, unit)}, latest {w.latest === null ? '—' : formatWeight(w.latest, unit)}, with a 7-day rolling average."
>
	{#snippet figure()}
		{#if w.change !== null}
			<CountUp value={Math.abs(display(w.change))} decimals={1} prefix={w.change < 0 ? '−' : '+'} />
			<small>{unit}</small>
		{:else if w.latest !== null}
			<CountUp value={display(w.latest)} decimals={1} />
			<small>{unit}</small>
		{/if}
	{/snippet}
	{#snippet note()}
		{#if w.monthChange !== null || w.change !== null}
			<strong>{signedWeightDelta(w.monthChange ?? w.change!, unit)}</strong>
			{w.monthChange !== null ? monthWord : 'this range'}
		{/if}
		{#if goal}{#if w.monthChange !== null || w.change !== null} · {/if}{goal}{/if}
	{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch" style="--c: var(--weight)"></i>Weigh-in</span>
	{/snippet}
	<div class="chart-wrap">
		{#key days.length}
			<svg
				bind:this={svg}
				class="chart"
				viewBox="0 0 100 100"
				preserveAspectRatio="none"
				aria-hidden="true"
				onpointermove={(e) => e.pointerType === 'mouse' && pick(e)}
				onpointerdown={pick}
			>
				<g class="grid-lines">
					{#each [25, 50, 75] as at}<line x1="0" x2="100" y1={at} y2={at} vector-effect="non-scaling-stroke" />{/each}
				</g>
				{#if areaPath}
					<path class="area" d={areaPath} />
					<path class="line grow-x" d={linePath} vector-effect="non-scaling-stroke" />
				{/if}
			</svg>
		{/key}
		<div class="axis" aria-hidden="true">
			{#each labels as l}<span>{l}</span>{/each}
		</div>
	</div>
</ChartCard>

<style>
	.grid-lines line {
		stroke: var(--chart-grid);
		stroke-width: 1;
	}
	.line {
		fill: none;
		stroke: var(--chart-weight);
		stroke-width: 2.5;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.area {
		fill: color-mix(in srgb, var(--chart-weight) 14%, transparent);
	}
	@media (prefers-reduced-motion: no-preference) {
		.area {
			animation: area-in var(--dur-slow) var(--ease) 120ms both;
		}
	}
	@keyframes area-in {
		from {
			opacity: 0;
		}
	}
</style>
