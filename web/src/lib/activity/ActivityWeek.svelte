<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivityWeek — a fixed seven-day window: steps as bars (scaled to the week's highest or your
  typical day, with a dashed line at the typical day), a dot on gym days, a count of synced
  workouts. Tapping or focusing a day reads it out in the line under the title; tapping also
  selects it (writes `day.date`) without moving the window. Props:
    date: string                 the selected day, highlighted in the window
    end: string                  the last day of the fixed window
    win: ActivityWindow | null   from ActivityDay's `win`; null shows a skeleton
-->
<script lang="ts">
	import { dayLabel } from '#lib/date.ts';
	import { day, session, today } from '#lib/session.svelte.ts';
	import { groupByDate, gymDates, weekBars, weekDayLines, weekWindow, windowCovers, type ActivityWindow } from './logic.ts';

	let { date, end, win }: { date: string; end: string; win: ActivityWindow | null } = $props();

	const typical = $derived(session.profile?.version?.typical_steps ?? null);
	const cols = $derived.by(() => {
		if (!win || !windowCovers(win, end, 30)) return null;
		const gym = gymDates(win.days, win.workouts);
		const byDay = groupByDate(win.workouts);
		const rows = weekWindow(end).map((d) => {
			const row = win.days.find((x) => x.date === d);
			return {
				date: d,
				steps: row?.steps ?? null,
				gym: gym.has(d),
				gym_minutes: row?.gym_minutes ?? null,
				trained: row?.trained ?? [],
				workouts: (byDay.get(d) ?? []).map((w) => w.name || w.type_label)
			};
		});
		const bars = weekBars(rows, typical);
		return {
			rows: rows.map((r, i) => ({ ...r, h: bars.bars[i].h })),
			typicalAt: bars.typicalAt,
			sessions: rows.filter((r) => r.gym).length,
			workouts: rows.reduce((n, r) => n + r.workouts.length, 0)
		};
	});

	const weekday = (d: string) => new Intl.DateTimeFormat('en-GB', { timeZone: 'UTC', weekday: 'short' }).format(new Date(`${d}T00:00:00Z`));
	const dom = (d: string) => Number(d.slice(8));

	type Row = { date: string; steps: number | null; gym_minutes: number | null; trained: string[]; workouts: string[] };
	const label = (r: Row) => `${dayLabel(r.date, today())}: ${weekDayLines(r).join(', ')}`;

	// The readout follows the focused day, and falls back to the selected one.
	let focused = $state<string | null>(null);
	const reading = $derived(cols?.rows.find((r) => r.date === (focused ?? date)) ?? null);
</script>

<section class="card week" aria-labelledby="week-h" aria-busy={cols === null}>
	<div class="card-head">
		<h2 id="week-h">This week</h2>
		{#if cols}
			<span>
				{cols.sessions} gym {cols.sessions === 1 ? 'day' : 'days'} · {cols.workouts}
				{cols.workouts === 1 ? 'workout' : 'workouts'}
			</span>
		{/if}
	</div>
	{#if !cols}
		<div class="skeleton skel" aria-hidden="true"></div>
	{:else}
		<p class="readout" aria-live="polite">{reading ? label(reading) : ''}</p>
		<ul class="days">
			{#each cols.rows as r, i (r.date)}
				<li>
					<button
						type="button"
						class="col"
						data-date={r.date}
						aria-label={label(r)}
						aria-current={r.date === date ? 'date' : undefined}
						onclick={() => (day.date = r.date)}
						onfocus={() => (focused = r.date)}
						onblur={() => (focused = null)}
					>
						<span class="plot" aria-hidden="true">
							{#if cols.typicalAt !== null}<span class="typical" style="bottom: {cols.typicalAt * 100}%"></span>{/if}
							{#if r.h === null}
								<span class="bar none"></span>
							{:else}
								<span class="bar grow-y" style="height: {Math.max(r.h * 100, 2)}%; --n: {i}"></span>
							{/if}
						</span>
						<span class="mark" aria-hidden="true">{#if r.gym}<i class="dot"></i>{/if}</span>
						<span class="count num" class:zero={!r.workouts.length} aria-hidden="true">{r.workouts.length || ''}</span>
						<span class="dow" aria-hidden="true">{weekday(r.date)}</span>
						<span class="dom num" aria-hidden="true">{dom(r.date)}</span>
					</button>
				</li>
			{/each}
		</ul>
		<p class="key small muted">
			<span><span class="k-bar"></span>Steps</span>
			{#if typical}<span><span class="k-line"></span>Typical day</span>{/if}
			<span><i class="dot"></i> Gym day</span>
			<span><span class="num k-n">2</span> Workouts</span>
		</p>
	{/if}
</section>

<style>
	.week {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.skel {
		height: 9rem;
		border-radius: var(--radius-md);
	}
	.readout {
		min-height: 1.5em;
		margin: 0;
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
	.days {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(7, minmax(0, 1fr));
		gap: var(--space-1);
	}
	.col {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;
		gap: 0.125rem;
		width: 100%;
		min-height: var(--touch);
		padding: var(--space-2) 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-muted);
		font: inherit;
		font-size: var(--text-xs);
		cursor: pointer;
		transition: background-color var(--dur-fast) var(--ease);
	}
	.col:hover {
		background: var(--surface-2);
	}
	.col:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.col[aria-current='date'] {
		background: var(--surface-2);
		color: var(--text);
		font-weight: var(--weight-bold);
	}
	.plot {
		position: relative;
		display: flex;
		align-items: flex-end;
		justify-content: center;
		width: 100%;
		height: 6rem;
	}
	.bar {
		width: 55%;
		max-width: 1.75rem;
		background: var(--steps);
		opacity: 0.6;
		border-radius: var(--radius-sm) var(--radius-sm) 2px 2px;
	}
	.col[aria-current='date'] .bar {
		opacity: 1;
	}
	/* No steps logged: a thin base tick, not a zero bar. */
	.bar.none {
		height: 3px;
		opacity: 1;
		background: var(--border-strong);
		border-radius: 1px;
	}
	.typical {
		position: absolute;
		left: 0;
		right: 0;
		border-top: 1.5px dashed var(--text-muted);
	}
	/* Two fixed rows under each bar, so dots and counts line up across days and never touch. */
	.mark {
		display: flex;
		align-items: center;
		height: 0.75rem;
		margin-top: var(--space-1);
	}
	.dot {
		display: inline-block;
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 50%;
		background: var(--accent);
	}
	.count {
		min-width: 1.5rem;
		height: 1.25rem;
		padding: 0 0.25rem;
		margin-bottom: var(--space-1);
		border-radius: var(--radius-pill);
		background: var(--surface-2);
		color: var(--text);
		line-height: 1.25rem;
		text-align: center;
	}
	.col[aria-current='date'] .count {
		background: var(--surface);
	}
	.count.zero {
		visibility: hidden;
	}
	.dom {
		line-height: 1;
	}
	.key {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		margin: 0;
	}
	.k-bar,
	.k-line {
		display: inline-block;
		margin-right: 0.25rem;
	}
	.k-bar {
		width: 0.625rem;
		height: 0.625rem;
		background: var(--steps);
		border-radius: 2px;
	}
	.k-line {
		width: 0.875rem;
		border-top: 1.5px dashed var(--text-muted);
		vertical-align: middle;
	}
	.k-n {
		color: var(--text);
	}
</style>
