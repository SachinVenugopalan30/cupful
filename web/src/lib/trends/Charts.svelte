<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Charts — Intake, Weight and Steps for Trends, drawn as small SVG components. The bars and the
  weight area animate from CSS on mount; the page remounts this block when the range changes, so
  the motion runs again. The line never uses dash offsets, so it stays continuous.
  Colors are tokens (--chart-*); the tapped day stays in the readout after the finger lifts.
-->
<script lang="ts">
	import type { TrendDay } from '../api';
	import { formatWeight, type WeightUnit } from '../format';
	import ChartCard from './ChartCard.svelte';
	import CountUp from './CountUp.svelte';
	import {
		averages,
		formatNumber,
		intakeReadout,
		rollingAverage,
		shortDate,
		signedWeightDelta,
		stepsReadout,
		stepsStats,
		tickDates,
		weightReadout,
		weightStats,
		goalRemaining
	} from './logic.ts';

	let {
		days,
		today,
		unit,
		typicalSteps,
		goalKg
	}: {
		days: TrendDay[];
		today: string;
		unit: WeightUnit;
		typicalSteps: number | null;
		goalKg: number | null;
	} = $props();

	// ---- shared ----
	const avg = $derived(averages(days));
	const rangeText = $derived(
		days.length ? `${shortDate(days[0].date)} to ${shortDate(days.at(-1)!.date)}` : ''
	);
	const isOver = (d: TrendDay) => d.budget_kcal !== null && d.eaten_kcal! > d.budget_kcal;
	const displayWeight = (kg: number) => (unit === 'kg' ? kg : kg / 0.45359237);

	/** Polyline point runs for a value that may be absent: gaps are gaps, never zeros. */
	function segments(pick: (d: TrendDay) => number | null, max: number): string[] {
		const runs: string[] = [];
		let run: string[] = [];
		days.forEach((d, i) => {
			const v = pick(d);
			if (v === null) {
				if (run.length) runs.push(run.join(' '));
				run = [];
				return;
			}
			run.push(`${i + 0.5},${100 - (v / max) * 100}`);
		});
		if (run.length) runs.push(run.join(' '));
		return runs;
	}

	// ---- intake ----
	const logged = $derived(days.filter((d) => d.eaten_kcal !== null));
	const overDays = $derived(logged.filter(isOver).length);
	const intakeMax = $derived(
		Math.max(1, ...days.flatMap((d) => [d.eaten_kcal ?? 0, d.budget_kcal ?? 0])) * 1.05
	);
	const intakeTicks = $derived(tickDates(days.map((d) => d.date), 3));
	const budgetRuns = $derived(segments((d) => d.budget_kcal, intakeMax));
	let intakeSel = $state<TrendDay | null>(null);
	const intakeSummary = $derived(
		`Calories eaten each day, ${rangeText}, against the daily budget. ${logged.length} days with meals, average ${Math.round(avg.intake ?? 0)} kcal against an average budget of ${Math.round(avg.budget ?? 0)} kcal; over budget on ${overDays} of them.`
	);

	// ---- weight ----
	const w = $derived(weightStats(days));
	const rolling = $derived(rollingAverage(days.map((d) => d.weight_kg)));
	const weighed = $derived(
		days.map((d, i) => ({ d, i })).filter((x) => x.d.weight_kg !== null)
	);
	const wRange = $derived.by(() => {
		const values = weighed.map((x) => x.d.weight_kg!);
		if (!values.length) return { lo: 0, hi: 1 };
		const lo = Math.min(...values);
		const hi = Math.max(...values);
		const pad = Math.max(0.3, (hi - lo) * 0.15);
		return { lo: lo - pad, hi: hi + pad };
	});
	const wx = (i: number) => (days.length > 1 ? (i / (days.length - 1)) * 100 : 50);
	const wy = (kg: number) => 100 - ((kg - wRange.lo) / (wRange.hi - wRange.lo)) * 100;
	const wPoints = $derived(weighed.map((x) => ({ x: wx(x.i), y: wy(x.d.weight_kg!) })));
	const linePath = $derived(wPoints.map((p, i) => `${i ? 'L' : 'M'}${p.x},${p.y}`).join(' '));
	const areaPath = $derived(
		wPoints.length
			? `M${wPoints[0].x},100 ${wPoints.map((p) => `L${p.x},${p.y}`).join(' ')} L${wPoints.at(-1)!.x},100 Z`
			: ''
	);
	const goal = $derived(goalRemaining(w.latest, goalKg, unit));
	const monthWord = $derived(days.length >= 30 ? 'this month' : 'this range');
	let weightSel = $state<{ d: TrendDay; i: number } | null>(null);
	let weightSvg: SVGSVGElement | undefined = $state();

	function pickWeight(e: PointerEvent) {
		if (!weightSvg || !weighed.length) return;
		const box = weightSvg.getBoundingClientRect();
		const target = Math.round(
			Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) * (days.length - 1)
		);
		weightSel = weighed.reduce((best, x) =>
			Math.abs(x.i - target) < Math.abs(best.i - target) ? x : best
		);
	}

	// ---- steps ----
	const s = $derived(stepsStats(days, typicalSteps));
	const stepMax = $derived(Math.max(1, s.best ?? 0, typicalSteps ?? 0) * 1.05);
	const typicalY = $derived(
		typicalSteps !== null && typicalSteps > 0 ? 100 - (typicalSteps / stepMax) * 100 : null
	);
	const stepTicks = $derived(tickDates(days.map((d) => d.date), 3));
	let stepSel = $state<TrendDay | null>(null);
	const stepNote = $derived.by(() => {
		if (s.avg === null) return '';
		const best = `best ${formatNumber(s.best ?? 0)}`;
		if (typicalSteps === null || typicalSteps <= 0 || s.diff === null)
			return `Best ${formatNumber(s.best ?? 0)}`;
		const diff = Math.round(s.diff);
		const gap =
			diff === 0
				? `on your ${formatNumber(typicalSteps)} typical day`
				: `${formatNumber(Math.abs(diff))} ${diff < 0 ? 'below' : 'above'} your ${formatNumber(typicalSteps)} typical day`;
		return `${gap} · ${best}`;
	});
	const stepsSummary = $derived(
		`Steps each day, ${rangeText}. ${s.count} days recorded, average ${Math.round(s.avg ?? 0)} steps, best ${s.best ?? 0}.`
	);
</script>

<ChartCard
	title="Intake"
	empty={logged.length
		? ''
		: 'No meals logged in this range. Log a meal on Today and its day shows up here.'}
	readout={intakeSel ? intakeReadout(intakeSel, today) : ''}
	summary={intakeSummary}
>
	{#snippet figure()}
		{#if avg.intake !== null}
			<CountUp value={avg.intake} /> <small>avg</small>
		{/if}
	{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch intake"></i>Within budget</span>
		<span class="key"><i class="swatch over"></i>Over budget</span>
		<span class="key"><i class="swatch tick"></i>Budget</span>
	{/snippet}
	<div class="chart-wrap">
		<svg
			class="chart bars-svg"
			viewBox="0 0 {days.length} 100"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{#each days as d, i (d.date)}
				{#if d.eaten_kcal !== null}
					<!-- Pointer readout only; the card's summary is the keyboard path (design.md). -->
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<rect
						role="presentation"
						class="bar"
						class:over={isOver(d)}
						x={i + 0.15}
						y={100 - (d.eaten_kcal / intakeMax) * 100}
						width={0.7}
						height={(d.eaten_kcal / intakeMax) * 100}
						style:--i={Math.min(i, 40)}
						onpointerenter={() => (intakeSel = d)}
						onpointerdown={() => (intakeSel = d)}
						onclick={() => (intakeSel = d)}
					/>
				{/if}
			{/each}
			<g class="target">
				{#each budgetRuns as points}
					<polyline {points} vector-effect="non-scaling-stroke" />
				{/each}
			</g>
		</svg>
		<div class="axis" aria-hidden="true">
			{#each intakeTicks as tick}<span>{shortDate(tick)}</span>{/each}
		</div>
	</div>
</ChartCard>

<ChartCard
	title="Weight"
	empty={weighed.length
		? ''
		: 'No weigh-ins in this range. Add one under Activity and the trend starts here.'}
	readout={weightSel ? weightReadout(weightSel.d, rolling[weightSel.i], today, unit) : ''}
	summary="Weigh-ins {rangeText}: {w.count} in all, first {w.first === null
		? '—'
		: formatWeight(w.first, unit)}, latest {w.latest === null
		? '—'
		: formatWeight(w.latest, unit)}, with a 7-day rolling average."
>
	{#snippet figure()}
		{#if w.change !== null}
			<CountUp
				value={Math.abs(displayWeight(w.change))}
				decimals={1}
				prefix={w.change < 0 ? '−' : '+'}
			/>
			<small>{unit}</small>
		{:else if w.latest !== null}
			<CountUp value={displayWeight(w.latest)} decimals={1} />
			<small>{unit}</small>
		{/if}
	{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch weight"></i>Weigh-in</span>
	{/snippet}
	{#snippet note()}
		{#if w.monthChange !== null || w.change !== null}
			<strong>{signedWeightDelta(w.monthChange ?? w.change!, unit)}</strong>
			{w.monthChange !== null ? monthWord : 'this range'}
		{/if}
		{#if goal}
			{#if w.monthChange !== null || w.change !== null} · {/if}{goal}
		{/if}
	{/snippet}
	<div class="chart-wrap">
		<svg
			bind:this={weightSvg}
			class="chart weight-svg"
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			aria-hidden="true"
			onpointermove={(e) => e.pointerType === 'mouse' && pickWeight(e)}
			onpointerdown={pickWeight}
		>
			<g class="grid-lines">
				<line x1="0" x2="100" y1="25" y2="25" vector-effect="non-scaling-stroke" />
				<line x1="0" x2="100" y1="50" y2="50" vector-effect="non-scaling-stroke" />
				<line x1="0" x2="100" y1="75" y2="75" vector-effect="non-scaling-stroke" />
			</g>
			{#if areaPath}
				<path class="area" d={areaPath} />
				<path class="line" d={linePath} vector-effect="non-scaling-stroke" />
			{/if}
		</svg>
		<div class="axis" aria-hidden="true">
			{#each tickDates(days.map((d) => d.date), 3) as tick}<span>{shortDate(tick)}</span>{/each}
		</div>
	</div>
</ChartCard>

<ChartCard
	title="Steps"
	empty={s.count
		? ''
		: 'No steps recorded in this range. Add them under Activity, or set up the Shortcut in Settings.'}
	readout={stepSel ? stepsReadout(stepSel, today) : ''}
	summary={stepsSummary}
>
	{#snippet figure()}
		{#if s.avg !== null}
			<CountUp value={s.avg} /> <small>avg</small>
		{/if}
	{/snippet}
	{#snippet legend()}
		<span class="key"><i class="swatch steps"></i>Steps</span>
		{#if typicalY !== null}<span class="key"><i class="swatch tick"></i>Typical</span>{/if}
	{/snippet}
	{#snippet note()}
		{#if stepNote}{stepNote}{/if}
	{/snippet}
	<div class="chart-wrap">
		<svg
			class="chart bars-svg"
			viewBox="0 0 {days.length} 100"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{#each days as d, i (d.date)}
				{#if d.steps !== null}
					<!-- Pointer readout only; the card's summary is the keyboard path (design.md). -->
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<rect
						role="presentation"
						class="bar step"
						x={i + 0.15}
						y={100 - (d.steps / stepMax) * 100}
						width={0.7}
						height={(d.steps / stepMax) * 100}
						style:--i={Math.min(i, 40)}
						onpointerenter={() => (stepSel = d)}
						onpointerdown={() => (stepSel = d)}
						onclick={() => (stepSel = d)}
					/>
				{/if}
			{/each}
			{#if typicalY !== null}
				<line
					class="typical"
					x1="0"
					x2={days.length}
					y1={typicalY}
					y2={typicalY}
					vector-effect="non-scaling-stroke"
				/>
			{/if}
		</svg>
		<div class="axis" aria-hidden="true">
			{#each stepTicks as tick}<span>{shortDate(tick)}</span>{/each}
		</div>
	</div>
</ChartCard>

<style>
	.chart-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	.chart {
		flex: 1;
		width: 100%;
		min-height: 0;
		overflow: visible;
	}
	.axis {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-xs);
		color: var(--chart-axis);
		padding-top: var(--space-1);
	}
	/* Legend keys: a shape plus a word, so color is never the only signal. */
	.key {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.swatch {
		display: inline-block;
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 0.125rem;
		background: var(--chart-intake);
	}
	.swatch.over {
		background: var(--chart-over);
	}
	.swatch.steps {
		background: var(--chart-steps);
	}
	.swatch.weight {
		background: var(--chart-weight);
	}
	.swatch.tick {
		width: 1rem;
		height: 3px;
		border-radius: 0;
		background: var(--chart-budget);
	}

	/* Bars grow from the baseline with a 12 ms stagger. The delay is capped so 365 days stay fast. */
	.bars-svg .bar {
		transform-box: fill-box;
		transform-origin: bottom;
		animation: grow-y 520ms var(--ease) both;
		animation-delay: calc(var(--i) * 12ms);
		fill: var(--chart-intake);
	}
	.bars-svg .bar.over {
		fill: var(--chart-over);
	}
	.bars-svg .bar.step {
		fill: var(--chart-steps);
	}
	.bars-svg .target polyline {
		fill: none;
		stroke: var(--chart-budget);
		stroke-width: 2;
		stroke-dasharray: 4 3;
		opacity: 0.75;
	}
	.typical {
		stroke: var(--chart-budget);
		stroke-width: 1.5;
		stroke-dasharray: 4 3;
		opacity: 0.55;
	}
	.weight-svg .grid-lines line {
		stroke: var(--chart-grid);
		stroke-width: 1;
	}
	.weight-svg .line {
		fill: none;
		stroke: var(--chart-weight);
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.weight-svg .area {
		fill: color-mix(in srgb, var(--chart-weight) 14%, transparent);
		animation: fade-in 450ms var(--ease) 300ms both;
	}
	@keyframes grow-y {
		from {
			opacity: 0.35;
			transform: scaleY(0.02);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	@keyframes fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bars-svg .bar,
		.weight-svg .area {
			animation: none;
		}
	}
</style>
