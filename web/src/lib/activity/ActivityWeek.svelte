<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivityWeek — the seven days ending on the selected one: steps as bars (scaled to the week's
  highest, with a dashed line at your typical day), a dot on gym days, a count of synced workouts.
  Tapping a day selects it (writes `day.date`). Props:
    date: string                 the selected day, the last column
    win: ActivityWindow | null   from ActivitySection's `win`; null shows a skeleton
-->
<script lang="ts">
	import { dayLabel } from '#lib/date.ts';
	import { day, session, today } from '#lib/session.svelte.ts';
	import { groupByDate, gymDates, weekWindow, type ActivityWindow } from './logic.ts';

	let { date, win }: { date: string; win: ActivityWindow | null } = $props();

	const typical = $derived(session.profile?.version?.typical_steps ?? null);
	const cols = $derived.by(() => {
		if (!win || win.date !== date) return null;
		const gym = gymDates(win.days, win.workouts);
		const byDay = groupByDate(win.workouts);
		const rows = weekWindow(date).map((d) => ({
			date: d,
			steps: win.days.find((x) => x.date === d)?.steps ?? null,
			gym: gym.has(d),
			workouts: byDay.get(d)?.length ?? 0
		}));
		const max = Math.max(typical ?? 0, ...rows.map((r) => r.steps ?? 0), 1);
		return { rows, max, sessions: rows.filter((r) => r.gym).length, workouts: rows.reduce((n, r) => n + r.workouts, 0) };
	});

	const weekday = (d: string) => new Intl.DateTimeFormat('en-GB', { timeZone: 'UTC', weekday: 'short' }).format(new Date(`${d}T00:00:00Z`));
	const dom = (d: string) => Number(d.slice(8));

	function label(r: { date: string; steps: number | null; gym: boolean; workouts: number }) {
		const parts = [r.steps === null ? 'no steps logged' : `${r.steps.toLocaleString()} steps`];
		if (r.gym) parts.push('gym');
		if (r.workouts) parts.push(`${r.workouts} ${r.workouts === 1 ? 'workout' : 'workouts'}`);
		return `${dayLabel(r.date, today())}: ${parts.join(', ')}`;
	}
</script>

<section class="card week" aria-labelledby="week-h" aria-busy={cols === null}>
	<header class="head">
		<h3 id="week-h">This week</h3>
		{#if cols}
			<p class="small muted">
				{cols.sessions} gym {cols.sessions === 1 ? 'day' : 'days'} · {cols.workouts}
				{cols.workouts === 1 ? 'workout' : 'workouts'}
			</p>
		{/if}
	</header>
	{#if !cols}
		<div class="skeleton skel" aria-hidden="true"></div>
	{:else}
		<ul class="days">
			{#each cols.rows as r (r.date)}
				<li>
					<button
						type="button"
						class="col"
						data-date={r.date}
						aria-label={label(r)}
						aria-current={r.date === date ? 'date' : undefined}
						onclick={() => (day.date = r.date)}
					>
						<span class="plot" aria-hidden="true">
							{#if typical}<span class="typical" style="bottom: {(typical / cols.max) * 100}%"></span>{/if}
							{#if r.steps === null}
								<span class="bar none"></span>
							{:else}
								<span class="bar" style="height: {Math.max((r.steps / cols.max) * 100, 2)}%"></span>
							{/if}
						</span>
						<span class="mark" aria-hidden="true">{#if r.gym}<i class="dot"></i>{/if}</span>
						<span class="count num" aria-hidden="true">{r.workouts || ''}</span>
						<span class="dow" aria-hidden="true">{weekday(r.date)}</span>
						<span class="dom num" aria-hidden="true">{dom(r.date)}</span>
					</button>
				</li>
			{/each}
		</ul>
		<p class="key small muted">
			<span><span class="k-bar"></span>Steps</span>
			<span><span class="k-line"></span>Typical day</span>
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
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
	}
	.head h3 {
		font-size: var(--text-lg);
		letter-spacing: -0.01em;
	}
	.skel {
		height: 9rem;
		border-radius: var(--radius-md);
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
		height: 5rem;
	}
	.bar {
		width: 55%;
		max-width: 1.75rem;
		background: var(--steps);
		border-radius: var(--radius-sm) var(--radius-sm) 2px 2px;
		transition: height var(--dur-slow) var(--ease);
	}
	/* No steps logged: a thin base tick, not a zero bar. */
	.bar.none {
		height: 2px;
		background: var(--border-strong);
		border-radius: 1px;
	}
	.typical {
		position: absolute;
		left: 0;
		right: 0;
		border-top: 1px dashed var(--text-muted);
	}
	.mark {
		display: flex;
		align-items: center;
		height: 0.75rem;
	}
	.dot {
		display: inline-block;
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 50%;
		background: var(--accent);
	}
	.count {
		min-height: 1em;
		line-height: 1;
	}
	.dom {
		line-height: 1;
	}
	.key {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
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
		border-top: 1px dashed var(--text-muted);
		vertical-align: middle;
	}
	.k-n {
		color: var(--text);
	}
	@media (prefers-reduced-motion: reduce) {
		.bar,
		.col {
			transition: none;
		}
	}
</style>
