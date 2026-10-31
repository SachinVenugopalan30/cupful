<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivitySection — steps, gym minutes, body parts trained and weigh-in for one day, each with
  the source of its value. A field saves when it loses focus (or on Enter); a chip saves on tap.
  Props (Today and the Activity tab pass these):
    date: string             YYYY-MM-DD, the selected day
    refreshKey: number       bumped after any write to the day; refetch when it changes
    onChanged: () => void    call after a write is applied (the budget card refreshes)
  Data: getDay(date). Writes only through #lib/sync (dayPatch + sendOps).
  The server never lets Shortcut or import values overwrite a manual one; typing here is manual.
-->
<script lang="ts">
	import { errorMessage, getDay, type Day } from '#lib/api.ts';
	import { round, weightToKg } from '#lib/format.ts';
	import { session, today } from '#lib/session.svelte.ts';
	import { dayPatch, sendOps, type DayFields } from '#lib/sync/index.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import {
		BODY_PARTS,
		MAX_GYM,
		MAX_STEPS,
		parseCount,
		parseWeight,
		sourceLabel,
		toggle,
		type Parsed
	} from './logic.ts';

	let { date, refreshKey, onChanged }: { date: string; refreshKey: number; onChanged: () => void } =
		$props();

	let data = $state<Day | null>(null);
	let loadError = $state('');
	let retry = $state(0);
	let steps = $state('');
	let gym = $state('');
	let weight = $state('');
	let error = $state('');
	let status = $state('');
	let busy = $state(false);

	const unit = $derived(session.profile?.weight_unit ?? 'kg');
	const typicalSteps = $derived(session.profile?.version?.typical_steps ?? null);
	const future = $derived(date > today());
	const shown = (kg: number | null) => (kg === null ? '' : String(round(kg / weightToKg(1, unit), 1)));

	let loaded = ''; // the date `data` belongs to
	$effect(() => {
		const d = date;
		void refreshKey;
		void dataVersion.n;
		void retry;
		if (!d) return;
		if (d !== loaded) data = null; // a new day shows the skeleton; a refresh keeps the form
		loaded = d;
		let stale = false;
		loadError = '';
		getDay(d).then(
			(r) => {
				if (stale) return;
				data = r;
				steps = r.steps?.toString() ?? '';
				gym = r.gym_minutes?.toString() ?? '';
				weight = shown(r.weight_kg);
			},
			(e) => !stale && (loadError = errorMessage(e))
		);
		return () => (stale = true);
	});

	async function save(fields: DayFields, done: string) {
		error = status = '';
		busy = true;
		try {
			const [r] = await sendOps([dayPatch(date, fields)]);
			if (r.status === 'rejected') throw new Error(`Not saved: ${r.error ?? 'rejected'}.`);
			if (r.status === 'stale') {
				error = staleNotice('day');
				onChanged();
				return;
			}
			status = r.status === 'queued' ? `${done} Waiting to sync.` : done;
			onChanged();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	/** Saves a text field if it changed; a bad value shows its error and keeps what's typed. */
	function commit(was: number | null, parsed: Parsed, key: keyof DayFields, done: string) {
		if ('error' in parsed) return void (error = parsed.error);
		if (parsed.value === was) return void (error = '');
		save({ [key]: parsed.value }, done);
	}
</script>

<section class="activity" aria-labelledby="activity-h" aria-busy={data === null && !loadError}>
	<header class="head">
		<h2 id="activity-h">Activity</h2>
		<p class="saved small" role="status">
			{#key status}
				{#if status}<span class="saved-text">{status}</span>{/if}
			{/key}
		</p>
	</header>

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
			<section class="card stat stat-steps" aria-labelledby="steps-h">
				<div class="stat-head">
					<h3 id="steps-h">Steps</h3>
					{#if d.steps !== null}
						<span class="chip">{d.steps_source ? sourceLabel(d.steps_source) : 'Manual'}</span>
					{:else}
						<span class="chip">Not logged</span>
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
				<p class="caption">
					{#if typicalSteps !== null}
						Typical day: <span class="num">{typicalSteps.toLocaleString()}</span> steps.
					{:else}
						Steps for this day.
					{/if}
				</p>
			</section>

			<section class="card stat stat-gym" aria-labelledby="gym-h">
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
				<fieldset class="trained">
					<legend class="visually-hidden">Trained body parts</legend>
					<div class="parts">
						{#each BODY_PARTS as part (part)}
							{@const on = d.trained.includes(part)}
							<button
								type="button"
								class="part"
								aria-pressed={on}
								disabled={future || busy}
								onclick={() => save({ trained: toggle(d.trained, part) }, `${part} ${on ? 'removed' : 'added'}.`)}
								>{part}</button
							>
						{/each}
					</div>
				</fieldset>
				<p class="caption">Tap the parts you trained.</p>
			</section>

			<section class="card stat stat-weight" aria-labelledby="weight-h">
				<div class="stat-head">
					<h3 id="weight-h">Weigh-in</h3>
					{#if d.weight_kg !== null}
						<span class="chip">Manual</span>
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
				<p class="caption">Changes the budget from this day until your next weigh-in.</p>
			</section>
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

	.trained {
		border: 0;
		margin: 0;
		padding: 0;
		min-width: 0;
	}
	.parts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.part {
		min-height: var(--touch);
		padding: 0 var(--space-3);
		border: 1px solid var(--control-border);
		border-radius: var(--radius-pill);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		cursor: pointer;
		transition:
			background-color var(--dur-fast) var(--ease),
			border-color var(--dur-fast) var(--ease),
			color var(--dur-fast) var(--ease),
			opacity var(--dur-fast) var(--ease);
	}
	.part:hover:not(:disabled) {
		background: var(--surface-2);
	}
	/* Selected: accent soft fill, bold text and an accent outline, so it reads without color. */
	.part[aria-pressed='true'] {
		background: var(--accent-soft);
		border-color: var(--accent);
		color: var(--accent);
		box-shadow: inset 0 0 0 1px var(--accent);
		font-weight: var(--weight-bold);
	}
	.part:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
		border-radius: var(--radius-pill);
	}
	.part:disabled {
		opacity: 0.55;
		cursor: not-allowed;
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
		.figure input,
		.part {
			transition: none;
		}
	}
</style>
