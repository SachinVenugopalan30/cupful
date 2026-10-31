<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivitySection — steps, gym minutes, body parts trained and weigh-in for one day, each with
  the source of its value. A field saves when it loses focus (or on Enter); a body part saves when ticked in the "Trained" menu.
  Props (Today and the Activity tab pass these):
    date: string             YYYY-MM-DD, the selected day
    refreshKey: number       bumped after any write to the day; refetch when it changes
    onChanged: () => void    call after a write is applied (the budget card refreshes)
    heading?: boolean        show the section heading and saved status (default true)
    syncSteps?: boolean      show a Google Health sync button on the steps card (Today)
    win?: ActivityWindow|null  bindable: the 30 days of day rows and 7 days of workouts around
                             `date`, for the week strip and workouts panel to share one fetch
  Data: getDay(date). Writes only through #lib/sync (dayPatch + sendOps).
  The server never lets Shortcut or import values overwrite a manual one; typing here is manual.
-->
<script lang="ts">
	import {
		errorMessage,
		getDay,
		getDays,
		getWorkouts,
		google,
		type Day,
		type GoogleStatus
	} from '#lib/api.ts';
	import { addDays, dayLabel } from '#lib/date.ts';
	import { formatWeight, round, weightToKg } from '#lib/format.ts';
	import { session, today } from '#lib/session.svelte.ts';
	import { dayPatch, sendOps, type DayFields } from '#lib/sync/index.ts';
	import { dismiss } from '#lib/dismiss.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import {
		BODY_PARTS,
		gymDates,
		lastTrained,
		lastWeight,
		MAX_GYM,
		MAX_STEPS,
		parseCount,
		parseWeight,
		sessionsIn,
		sourceLabel,
		sparklinePoints,
		stepsAverage,
		stepsVsTypical,
		toggle,
		weekWindow,
		weightDelta,
		type ActivityWindow,
		type Parsed
	} from './logic.ts';

	let {
		date,
		refreshKey,
		onChanged,
		heading = true,
		syncSteps = false,
		win = $bindable(null)
	}: {
		date: string;
		refreshKey: number;
		onChanged: () => void;
		heading?: boolean;
		syncSteps?: boolean;
		win?: ActivityWindow | null;
	} = $props();

	let data = $state<Day | null>(null);
	let loadError = $state('');
	let retry = $state(0);
	let steps = $state('');
	let gym = $state('');
	let weight = $state('');
	let error = $state('');
	let status = $state('');
	let trained = $state<string[]>([]);
	let googleStatus = $state<GoogleStatus | null>(null);
	let syncing = $state(false);
	let syncNote = $state('');

	const unit = $derived(session.profile?.weight_unit ?? 'kg');
	const typicalSteps = $derived(session.profile?.version?.typical_steps ?? null);
	const future = $derived(date > today());

	// Facts under the figures. They come from the window and only show once it is for this date.
	const w = $derived(win && win.date === date ? win : null);
	const week = $derived(weekWindow(date));
	const pct = $derived(data ? stepsVsTypical(data.steps, typicalSteps) : null);
	const avg = $derived(w ? stepsAverage(w.days, week) : null);
	const gym7 = $derived(w ? gymDates(w.days, w.workouts) : null);
	const last = $derived(gym7 ? lastTrained(gym7, date) : null);
	const delta = $derived(w ? weightDelta(w.days, date) : null);
	const carried = $derived(
		data && data.weight_kg === null && w ? lastWeight(w.days, addDays(date, -1)) : null
	);
	const canSync = $derived(
		syncSteps &&
			googleStatus?.configured === true &&
			googleStatus.connected &&
			!googleStatus.needs_reconnect &&
			!future
	);
	const trend = $derived(
		w ? sparklinePoints(Array.from({ length: 30 }, (_, i) => w.days.find((x) => x.date === addDays(date, i - 29))?.weight_kg ?? null), 120, 32) : ''
	);
	const signed = (kg: number) => {
		const t = formatWeight(Math.abs(kg), unit);
		return Math.abs(round(kg / weightToKg(1, unit), 1)) === 0 ? `No change` : `${kg < 0 ? '−' : '+'}${t}`;
	};
	const shown = (kg: number | null) => (kg === null ? '' : String(round(kg / weightToKg(1, unit), 1)));

	$effect(() => {
		if (!syncSteps) return;
		google.status().then(
			(s) => (googleStatus = s),
			() => (googleStatus = null)
		);
	});

	$effect(() => {
		if (carried && weight === '') weight = shown(carried.kg);
	});

	let loaded = ''; // the date `data` belongs to
	$effect(() => {
		const d = date;
		void refreshKey;
		void dataVersion.n;
		void retry;
		if (!d) return;
		if (d !== loaded) {
			data = null; // a new day shows the skeleton; a refresh keeps the form
			win = null;
		}
		loaded = d;
		let stale = false;
		loadError = '';
		// The window never blocks the form: if it fails, the cards simply show no extras. Workouts
		// fail separately (the endpoint may be empty or absent) and then read as none.
		Promise.all([
			getDays(addDays(d, -29), d).catch(() => [] as Day[]),
			getWorkouts(addDays(d, -6), d).catch(() => [])
		]).then(([days, workouts]) => !stale && (win = { date: d, days, workouts }));
		getDay(d).then(
			(r) => {
				if (stale) return;
				data = r;
				steps = r.steps?.toString() ?? '';
				gym = r.gym_minutes?.toString() ?? '';
				weight = shown(r.weight_kg);
				trained = r.trained;
			},
			(e) => !stale && (loadError = errorMessage(e))
		);
		return () => (stale = true);
	});

	async function save(fields: DayFields, done: string): Promise<boolean> {
		error = status = syncNote = '';
		try {
			const [r] = await sendOps([dayPatch(date, fields)]);
			if (r.status === 'rejected') throw new Error(`Not saved: ${r.error ?? 'rejected'}.`);
			if (r.status === 'stale') {
				error = staleNotice('day');
				onChanged();
				return false;
			}
			status = r.status === 'queued' ? `${done} Waiting to sync.` : done;
			onChanged();
			return true;
		} catch (e) {
			error = errorMessage(e);
			return false;
		}
	}

	async function syncFromGoogle() {
		syncing = true;
		error = status = syncNote = '';
		try {
			const r = await google.sync();
			status = r.changed > 0 ? 'Steps synced.' : 'Steps already up to date.';
			syncNote = status;
			onChanged();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			syncing = false;
		}
	}

	/** Ticks a body part at once and saves the list; a failed save puts the saved list back. */
	async function togglePart(part: string) {
		const on = trained.includes(part);
		trained = toggle(trained, part);
		if (!(await save({ trained }, `${part} ${on ? 'removed' : 'added'}.`)) && data) trained = data.trained;
	}

	/** Saves a text field if it changed; a bad value shows its error and keeps what's typed. */
	function commit(was: number | null, parsed: Parsed, key: keyof DayFields, done: string) {
		if ('error' in parsed) return void (error = parsed.error);
		if (parsed.value === was) return void (error = '');
		save({ [key]: parsed.value }, done);
	}
</script>

<section class="activity" aria-label="Activity" aria-busy={data === null && !loadError}>
	{#if heading}
		<header class="head">
			<h2>Activity</h2>
			<p class="saved small" role="status">
				{#key status}
					{#if status}<span class="saved-text">{status}</span>{/if}
				{/key}
			</p>
		</header>
	{:else}
		<p class="visually-hidden" role="status">
			{#key status}
				{#if status}<span class="saved-text">{status}</span>{/if}
			{/key}
		</p>
	{/if}

	{#if loadError}
		<div class="notice notice-error" role="alert">
			<p>{loadError}</p>
			<div class="row"><button class="btn" onclick={() => retry++}>Try again</button></div>
		</div>
	{:else if data === null}
		<div class="stats" aria-hidden="true">
			<div class="skeleton skeleton-card"></div>
			<div class="skeleton skeleton-card"></div>
			<div class="skeleton skeleton-card"></div>
		</div>
	{:else}
		{@const d = data}
		{#if future}<p class="notice notice-info">Future days can't be logged yet.</p>{/if}
		{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}

		<div class="stats">
			<div class="card stat stat-steps">
				<div class="stat-head">
					<h3 id="steps-h">Steps</h3>
					{#if d.steps !== null}
						<span class="chip" data-testid="steps-source">{d.steps_source ? sourceLabel(d.steps_source) : 'Manual'}</span>
					{:else}
						<span class="chip" data-testid="steps-source">Not logged</span>
					{/if}
				</div>
				<div class="figure">
					<label class="visually-hidden" for="act-steps">Steps</label>
					<input
						id="act-steps"
						class="num"
						inputmode="numeric"
						placeholder="0"
						bind:value={steps}
						disabled={future}
						onchange={() => commit(d.steps, parseCount(steps, MAX_STEPS, 'Steps'), 'steps', 'Steps saved.')}
					/>
					<span class="unit" aria-hidden="true">steps</span>
				</div>
				{#if canSync}
					<div class="sync-row">
						<button type="button" class="btn btn-quiet sync" onclick={syncFromGoogle} disabled={syncing}>
							{syncing ? 'Syncing…' : 'Sync steps'}
						</button>
						{#if !heading && syncNote}<span class="small muted">{syncNote}</span>{/if}
					</div>
				{/if}
				{#if pct !== null}
					<div
						class="meter"
						style="--value: {pct / 100}; --fill: var(--steps)"
						role="meter"
						aria-label="Steps against your typical day"
						aria-valuemin="0"
						aria-valuemax={typicalSteps}
						aria-valuenow={d.steps}
						aria-valuetext="{pct}% of your typical day"
					></div>
					<p class="fact"><span class="num">{pct}%</span> of your typical day</p>
				{/if}
				{#if avg !== null}
					<p class="fact">7-day average <span class="num">{Math.round(avg).toLocaleString()}</span></p>
				{/if}
				<p class="caption">
					{#if typicalSteps !== null}
						Typical day: <span class="num">{typicalSteps.toLocaleString()}</span> steps.
					{:else}
						Steps for this day.
					{/if}
				</p>
			</div>

			<div class="card stat stat-gym">
				<div class="stat-head">
					<h3 id="gym-h">Gym</h3>
					{#if d.gym_minutes !== null}
						<span class="chip">{d.gym_source ? sourceLabel(d.gym_source) : 'Manual'}</span>
					{:else}
						<span class="chip">Not logged</span>
					{/if}
				</div>
				<div class="figure">
					<label class="visually-hidden" for="act-gym">Gym minutes</label>
					<input
						id="act-gym"
						class="num"
						inputmode="numeric"
						placeholder="0"
						bind:value={gym}
						disabled={future}
						onchange={() => commit(d.gym_minutes, parseCount(gym, MAX_GYM, 'Gym minutes'), 'gym_minutes', 'Gym minutes saved.')}
					/>
					<span class="unit" aria-hidden="true">minutes</span>
				</div>
				<details class="menu trained" use:dismiss>
					<summary class="btn trained-btn" aria-label="Trained body parts: {trained.length ? trained.join(', ') : 'none'}">
						<span class="trained-text">Trained: {trained.length ? trained.join(', ') : 'none'}</span>
					</summary>
					<div class="menu-panel">
						<fieldset class="parts">
							<legend class="visually-hidden">Trained body parts</legend>
							{#each BODY_PARTS as part (part)}
								<label class="part">
									<input
										type="checkbox"
										checked={trained.includes(part)}
										disabled={future}
										onchange={() => togglePart(part)}
									/>
									{part}
								</label>
							{/each}
						</fieldset>
						<button type="button" class="btn done" data-close>Done</button>
					</div>
				</details>
				{#if gym7}
					{@const n = sessionsIn(gym7, week)}
					<p class="fact">
						<span class="num">{n}</span>
						{n === 1 ? 'session' : 'sessions'} in the last 7 days{#if last}; last trained {dayLabel(last, today())}{/if}
					</p>
				{/if}
				<p class="caption">Pick the parts you trained.</p>
			</div>

			<div class="card stat stat-weight">
				<div class="stat-head">
					<h3 id="weight-h">Weigh-in</h3>
					{#if d.weight_kg !== null}
						<span class="chip">Manual</span>
					{:else if carried}
						<span class="chip">Carried</span>
					{:else}
						<span class="chip">Not logged</span>
					{/if}
				</div>
				<div class="figure">
					<label class="visually-hidden" for="act-weight">Weigh-in ({unit})</label>
					<input
						id="act-weight"
						class="num"
						inputmode="decimal"
						placeholder="0.0"
						bind:value={weight}
						disabled={future}
						onchange={() =>
							commit(d.weight_kg, parseWeight(weight, unit), 'weight_kg', 'Weigh-in saved.')}
					/>
					<span class="unit" aria-hidden="true">{unit}</span>
				</div>
				{#if delta}
					<p class="fact">
						<span class="num">{signed(delta.deltaKg)}</span> since {dayLabel(delta.prevDate, today())}
					</p>
				{/if}
				{#if trend}
					<svg class="spark" viewBox="0 0 120 32" preserveAspectRatio="none" role="img" aria-label="Weigh-ins over the last 30 days">
						<polyline points={trend} />
					</svg>
				{/if}
				<p class="caption">
					{#if carried}Carried from {dayLabel(carried.date, today())}.{' '}{/if}Changes the budget from this day until your next weigh-in.
				</p>
			</div>
		</div>
	{/if}
</section>

<style>
	.activity {
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
		min-height: 1.5rem;
	}
	.head h2 {
		font-size: var(--text-lg);
	}
	.saved {
		min-width: 0;
		color: var(--text-muted);
		text-align: right;
	}
	.saved-text {
		animation: saved-in var(--dur-med) var(--ease);
	}
	@keyframes saved-in {
		from {
			opacity: 0;
		}
	}

	/* One column on phones; two with a wide weigh-in in the middle; three on desktop. */
	.stats {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-3);
	}
	.skeleton-card {
		min-height: 10rem;
		border-radius: var(--radius-lg);
	}

	.stat {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
		transition: border-color var(--dur-fast) var(--ease);
	}
	.stat:focus-within {
		border-color: var(--accent);
	}
	.stat-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: 1.5rem;
	}
	.stat-head h3 {
		font-size: var(--text-lg);
		letter-spacing: -0.01em;
	}

	.figure {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		min-height: var(--touch);
		min-width: 0;
	}
	.figure input {
		width: 6ch;
		field-sizing: content;
		min-width: 1ch;
		max-width: 100%;
		min-height: var(--touch);
		padding: 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text);
		font-family: var(--font-numeric);
		font-size: 2.375rem; /* 38px figure */
		font-weight: var(--weight-bold);
		line-height: 1;
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
		font-feature-settings: 'tnum' 1;
		transition: background-color var(--dur-fast) var(--ease);
	}
	.stat-gym .figure input {
		width: 3ch;
	}
	.stat-weight .figure input {
		width: 5ch;
	}
	.sync-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
	.figure input:hover:not(:disabled) {
		background: var(--surface-2);
	}
	.figure input:focus-visible {
		outline-offset: 2px;
		background: var(--surface-2);
	}
	.figure input::placeholder {
		color: var(--text-muted);
		opacity: 0.6;
	}
	.figure input:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
	.stat-steps .figure input {
		color: var(--steps);
	}
	.stat-weight .figure input {
		color: var(--weight);
	}
	.unit {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.caption {
		color: var(--text-muted);
		font-size: var(--text-xs);
		margin-top: auto;
	}

	.fact {
		font-size: var(--text-sm);
		color: var(--text);
	}
	.stat .meter {
		height: 0.5rem;
	}
	.spark {
		width: 100%;
		height: 2rem;
		overflow: visible;
	}
	.spark polyline {
		fill: none;
		stroke: var(--weight);
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
		vector-effect: non-scaling-stroke;
	}

	.trained-btn {
		justify-content: flex-start;
		width: 100%;
		min-width: 0;
		font-size: var(--text-sm);
	}
	.trained-text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.trained-btn::after {
		content: '';
		flex: none;
		width: 0.5rem;
		height: 0.5rem;
		margin-left: auto;
		border: solid currentColor;
		border-width: 0 2px 2px 0;
		transform: translateY(-2px) rotate(45deg);
	}
	.trained[open] .trained-btn::after {
		transform: translateY(2px) rotate(-135deg);
	}
	.parts {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 var(--space-2);
		margin: 0 0 var(--space-2);
		padding: 0;
		border: 0;
		min-width: 0;
	}
	.part {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--touch);
		font-size: var(--text-sm);
		cursor: pointer;
	}
	.part input {
		width: 1.125rem;
		height: 1.125rem;
		accent-color: var(--accent);
	}
	.part:has(input:disabled) {
		opacity: 0.55;
		cursor: not-allowed;
	}
	.done {
		width: 100%;
	}

	@media (min-width: 30rem) {
		.stats {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.stat-weight {
			grid-column: 1 / -1;
		}
	}
	@media (min-width: 46rem) {
		.stats {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
		.stat-weight {
			grid-column: auto;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.saved-text {
			animation: none;
		}
		.stat,
		.figure input {
			transition: none;
		}
	}
</style>
