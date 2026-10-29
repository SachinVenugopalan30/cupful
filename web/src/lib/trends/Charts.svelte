<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Charts — the three Trends charts, drawn with LayerChart (SVG layer). Loaded lazily by the
  Trends page so Today's bundle doesn't carry the library.
  Props:
    days: TrendDay[]   one entry per date, oldest first (GET /api/trends)
    today: string      the user's today, for "Today"/"Yesterday" in readouts
    unit: WeightUnit   display unit for weight
  Colors are tokens (--chart-*). The day last touched stays in the readout after the finger lifts.
-->
<script lang="ts">
	import 'layerchart/core.css';
	import type { ComponentProps } from 'svelte';
	import { BarChart, Bars, Line, LineChart, Points, Spline } from 'layerchart/svg';
	import type { TrendDay } from '#lib/api.ts';
	import { round, weightToKg, type WeightUnit } from '#lib/format.ts';
	import ChartCard from './ChartCard.svelte';
	import { averages, intakeReadout, rollingAverage, stepsReadout, tickDates, weightReadout } from './logic.ts';

	let { days, today, unit }: { days: TrendDay[]; today: string; unit: WeightUnit } = $props();

	type Ctx = ComponentProps<typeof BarChart>['context'];
	const fromTime = (t: Date) => t.toLocaleDateString(undefined, { day: 'numeric', month: 'short', timeZone: 'UTC' });
	const shortDate = (iso: string) => fromTime(new Date(iso));
	const xTicks = $derived(tickDates(days.map((d) => d.date), 5));
	const bandPadding = $derived(days.length > 120 ? 0 : days.length > 45 ? 0.1 : 0.3);
	const pad = { left: 40, bottom: 24, top: 8, right: 8 };
	const range = $derived(`${shortDate(days[0].date)} to ${shortDate(days[days.length - 1].date)}`);

	// ---- intake ----
	const logged = $derived(days.filter((d) => d.eaten_kcal !== null));
	const avg = $derived(averages(days));
	const isOver = (d: TrendDay) => d.budget_kcal !== null && d.eaten_kcal! > d.budget_kcal;
	const overDays = $derived(logged.filter(isOver).length);
	let intakeCtx = $state<Ctx>();
	let intakeDay = $state<TrendDay | null>(null);
	$effect(() => void (intakeCtx?.tooltip.data && (intakeDay = intakeCtx.tooltip.data)));

	// ---- weight (rows carry values in the display unit) ----
	const perUnit = $derived(weightToKg(1, unit));
	const rolling = $derived(rollingAverage(days.map((d) => d.weight_kg)));
	const wrows = $derived(
		days.map((d, i) => ({
			...d,
			t: new Date(d.date + 'T00:00:00Z'),
			w: d.weight_kg === null ? null : round(d.weight_kg / perUnit, 1),
			a: rolling[i] === null ? null : round(rolling[i]! / perUnit, 1)
		}))
	);
	type Row = (typeof wrows)[number];
	const weighed = $derived(wrows.filter((r) => r.w !== null));
	const yWeight = $derived.by(() => {
		const v = wrows.flatMap((r) => [r.w, r.a]).filter((x) => x !== null);
		return v.length ? [Math.floor(Math.min(...v) - 1), Math.ceil(Math.max(...v) + 1)] : [0, 1];
	});
	let weightCtx = $state<Ctx>();
	let weightRow = $state<Row | null>(null);
	$effect(() => void (weightCtx?.tooltip.data && (weightRow = weightCtx.tooltip.data)));

	// ---- steps ----
	const stepDays = $derived(days.filter((d) => d.steps !== null));
	let stepsCtx = $state<Ctx>();
	let stepsDay = $state<TrendDay | null>(null);
	$effect(() => void (stepsCtx?.tooltip.data && (stepsDay = stepsCtx.tooltip.data)));
</script>

<!-- Any tooltip snippet replaces LayerChart's default popup; the readout above each chart takes its place. -->
{#snippet tooltip()}{/snippet}

<ChartCard
	title="Intake"
	empty={logged.length ? '' : 'No meals logged in this range. Log a meal on Today and its day shows up here.'}
	readout={intakeDay ? intakeReadout(intakeDay, today) : ''}
	note="Past budgets use the weights recorded for those days."
	summary="Calories eaten each day, {range}, against the daily budget. {logged.length} days with meals, average {Math.round(avg.intake ?? 0)} kcal against an average budget of {Math.round(avg.budget ?? 0)} kcal; over budget on {overDays} of them."
>
	{#snippet legend()}
		<span class="key"><i class="swatch"></i>Eaten</span>
		<span class="key"><i class="swatch over"></i>Over budget</span>
		<span class="key"><i class="swatch tick"></i>Budget</span>
	{/snippet}
	<BarChart
		bind:context={intakeCtx}
		data={days}
		x="date"
		y="eaten_kcal"
		yDomain={[0, null]}
		yNice
		{bandPadding}
		padding={pad}
		{tooltip}
		props={{ xAxis: { ticks: xTicks, format: shortDate }, yAxis: { ticks: 4 } }}
	>
		{#snippet marks({ context })}
			<Bars data={logged.filter((d) => !isOver(d))} radius={days.length > 120 ? 0 : 2} fill="var(--chart-intake)" stroke="none" />
			<Bars data={logged.filter(isOver)} radius={days.length > 120 ? 0 : 2} fill="var(--chart-over)" stroke="none" />
			{#each logged as d (d.date)}
				{#if d.budget_kcal !== null}
					{@const x = context.xScale(d.date) as number}
					{@const y = context.yScale(d.budget_kcal) as number}
					{@const w = (context.xScale as unknown as { bandwidth(): number }).bandwidth()}
					<!-- A halo in the card color keeps the tick readable over any bar. -->
					<Line x1={x - 1} x2={x + w + 1} y1={y} y2={y} stroke="var(--surface)" strokeWidth={5} />
					<Line x1={x - 1} x2={x + w + 1} y1={y} y2={y} stroke="var(--chart-budget)" strokeWidth={2.5} />
				{/if}
			{/each}
		{/snippet}
	</BarChart>
</ChartCard>

<ChartCard
	title="Weight"
	empty={weighed.length ? '' : 'No weigh-ins in this range. Add one under Activity and the trend starts here.'}
	readout={weightRow ? weightReadout(weightRow, rolling[wrows.indexOf(weightRow)], today, unit) : ''}
	summary="Weigh-ins {range}: {weighed.length} in all, first {weighed[0]?.w} {unit}, latest {weighed.at(-1)?.w} {unit}, with a 7-day rolling average."
>
	{#snippet legend()}
		<span class="key"><i class="swatch dot"></i>Weigh-in</span>
		<span class="key"><i class="swatch line"></i>7-day average</span>
	{/snippet}
	<LineChart
		bind:context={weightCtx}
		data={wrows}
		x="t"
		y="a"
		yDomain={yWeight}
		padding={pad}
		{tooltip}
		tooltipContext={{ mode: 'bisect-x' }}
		props={{ xAxis: { ticks: 4, format: fromTime }, yAxis: { ticks: 4 } }}
	>
		{#snippet marks()}
			<Spline y="a" stroke="var(--chart-average)" strokeWidth={2.5} fill="none" />
			<Points
				data={weighed}
				y="w"
				r={days.length > 120 ? 2 : 3.5}
				fill="var(--chart-weight)"
				stroke="var(--surface)"
				strokeWidth={1}
			/>
		{/snippet}
	</LineChart>
</ChartCard>

<ChartCard
	title="Steps"
	empty={stepDays.length ? '' : 'No steps recorded in this range. Add them under Activity, or set up the Shortcut in Settings.'}
	readout={stepsDay ? stepsReadout(stepsDay, today) : ''}
	summary="Steps each day, {range}. {stepDays.length} days recorded, average {Math.round(avg.steps ?? 0)} steps."
>
	<BarChart
		bind:context={stepsCtx}
		data={days}
		x="date"
		y="steps"
		yDomain={[0, null]}
		yNice
		{bandPadding}
		padding={pad}
		{tooltip}
		props={{
			xAxis: { ticks: xTicks, format: shortDate },
			yAxis: { ticks: 4 },
			bars: { fill: "var(--chart-steps)", stroke: 'none', radius: days.length > 120 ? 0 : 2 }
		}}
	/>
</ChartCard>

<style>
	/* Legend keys: a shape plus a word, so color is never the only signal. */
	:global(.legend .key) {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	:global(.legend .swatch) {
		display: inline-block;
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 2px;
		background: var(--chart-intake);
	}
	:global(.legend .swatch.over) {
		background: var(--chart-over);
	}
	:global(.legend .swatch.tick),
	:global(.legend .swatch.line) {
		width: 1rem;
		height: 3px;
		border-radius: 0;
		background: var(--chart-budget);
	}
	:global(.legend .swatch.line) {
		background: var(--chart-average);
	}
	:global(.legend .swatch.dot) {
		border-radius: 50%;
		background: var(--chart-weight);
	}
</style>
