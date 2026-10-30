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
	const future = $derived(date > today());
	const shown = (kg: number | null) => (kg === null ? '' : String(round(kg / weightToKg(1, unit), 1)));

	let loaded = ''; // the date `data` belongs to
	$effect(() => {
		const d = date;
		void refreshKey;
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
			status = done;
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

<section class="card stack" aria-labelledby="activity-h" aria-busy={data === null && !loadError}>
	<h2 id="activity-h">Activity</h2>
	<p class="visually-hidden" role="status">{status}</p>

	{#if loadError}
		<div class="notice notice-error" role="alert">
			<p>{loadError}</p>
			<div class="row"><button class="btn" onclick={() => retry++}>Try again</button></div>
		</div>
	{:else if data === null}
		<div class="skeleton" style="height: 3rem"></div>
		<div class="skeleton" style="height: 3rem"></div>
		<div class="skeleton" style="height: 6rem"></div>
	{:else}
		{@const d = data}
		{#if future}<p class="notice notice-info">Future days can't be logged yet.</p>{/if}
		{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}

		<div class="pair">
			<div class="field">
				<span class="label-row">
					<label for="act-steps">Steps</label>
					{#if d.steps !== null && d.steps_source}<span class="chip">{sourceLabel(d.steps_source)}</span>{/if}
				</span>
				<input
					id="act-steps"
					class="num"
					inputmode="numeric"
					placeholder="0"
					bind:value={steps}
					disabled={future}
					onchange={() => commit(d.steps, parseCount(steps, MAX_STEPS, 'Steps'), 'steps', 'Steps saved.')}
				/>
			</div>
			<div class="field">
				<span class="label-row">
					<label for="act-gym">Gym minutes</label>
					{#if d.gym_minutes !== null && d.gym_source}<span class="chip">{sourceLabel(d.gym_source)}</span>{/if}
				</span>
				<input
					id="act-gym"
					class="num"
					inputmode="numeric"
					placeholder="0"
					bind:value={gym}
					disabled={future}
					onchange={() => commit(d.gym_minutes, parseCount(gym, MAX_GYM, 'Gym minutes'), 'gym_minutes', 'Gym minutes saved.')}
				/>
			</div>
		</div>

		<fieldset class="trained">
			<legend class="label">Trained</legend>
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

		<div class="field">
			<span class="label-row">
				<label for="act-weight">Weigh-in ({unit})</label>
				{#if d.weight_kg !== null}<span class="chip">Manual</span>{/if}
			</span>
			<input
				id="act-weight"
				class="num"
				inputmode="decimal"
				placeholder={unit}
				bind:value={weight}
				disabled={future}
				onchange={() =>
					commit(d.weight_kg, parseWeight(weight, unit), 'weight_kg', 'Weigh-in saved.')}
			/>
			<p class="hint">Changes the budget from this day until your next weigh-in.</p>
		</div>
	{/if}
</section>

<style>
	.pair {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
		gap: var(--space-3);
	}
	.label-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: 1.5rem;
	}
	.trained {
		border: 0;
		margin: 0;
		padding: 0;
		min-width: 0;
	}
	.trained legend {
		padding: 0;
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
	.parts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.part {
		min-height: var(--touch);
		padding: 0 var(--space-4);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-pill);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease);
	}
	.part:hover:not(:disabled) {
		background: var(--surface-2);
	}
	/* Selected: sage fill, bold text and an outline, so it reads without color. */
	.part[aria-pressed='true'] {
		background: var(--sage-soft);
		color: var(--sage-text);
		border-color: var(--sage);
		box-shadow: inset 0 0 0 1px var(--sage);
		font-weight: var(--weight-bold);
	}
	.part:focus-visible {
		outline: 3px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.part:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
</style>
