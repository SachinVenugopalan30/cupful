<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ProfileSection — profile and targets (saving creates a new profile version from today), units,
  timezone. Online only. Values are stored metric; the form shows the user's units.
-->
<script lang="ts">
	import { errorMessage, profile } from '#lib/api.ts';
	import { loadSession } from '#lib/session.svelte.ts';
	import { fromForm, setHeightUnit, setWeightUnit, toForm, type ProfileErrors, type ProfileForm } from './profile.ts';

	const browserTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
	const zones = Intl.supportedValuesOf('timeZone');
	const thisYear = new Date().getFullYear();

	let f = $state<ProfileForm | null>(null);
	let isNew = $state(false);
	let loadError = $state('');
	let errors = $state<ProfileErrors>({});
	let status = $state('');
	let saveError = $state('');
	let busy = $state(false);
	let online = $state(typeof navigator === 'undefined' || navigator.onLine);

	async function load() {
		loadError = '';
		try {
			const p = await profile.get();
			isNew = p.version === null;
			f = toForm(p, browserTz);
		} catch (e) {
			loadError = errorMessage(e);
		}
	}
	load();

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!f) return;
		status = saveError = '';
		const r = fromForm(f, thisYear);
		errors = r.errors;
		if (!r.value) return;
		busy = true;
		try {
			await profile.save(r.value);
			await loadSession();
			isNew = false;
			status = 'Profile saved. Your budget uses it from today.';
		} catch (err) {
			saveError = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	const invalid = (k: keyof ProfileErrors) => (errors[k] ? 'true' : undefined);
	const describe = (k: keyof ProfileErrors, hint?: string) =>
		[hint, errors[k] && `${k}-err`].filter(Boolean).join(' ') || undefined;
</script>

<svelte:window ononline={() => (online = true)} onoffline={() => (online = false)} />

<section class="card stack" aria-labelledby="profile-h">
	<h2 id="profile-h">Profile and targets</h2>
	{#if loadError}
		<div class="notice notice-error" role="alert">
			<p>{loadError}</p>
			<p><button class="btn" onclick={load}>Try again</button></p>
		</div>
	{:else if !f}
		<div class="stack" aria-busy="true">
			{#each [0, 1, 2, 3] as i (i)}<div class="skeleton field-skel"></div>{/each}
		</div>
	{:else}
		{#if isNew}
			<p class="notice notice-info">Fill this in to get your daily budget. Everything stays on this server.</p>
		{/if}
		{#if !online}
			<p class="notice notice-warning">You're offline. Profile changes need a connection.</p>
		{/if}
		<form class="stack" onsubmit={save} novalidate>
			<fieldset class="stack" disabled={!online || busy}>
				<legend class="visually-hidden">Profile</legend>
				<div class="field">
					<label for="p-tz">Time zone</label>
					<select id="p-tz" bind:value={f.timezone} aria-describedby="p-tz-hint">
						{#if !zones.includes(f.timezone)}<option>{f.timezone}</option>{/if}
						{#each zones as z (z)}<option>{z}</option>{/each}
					</select>
					<p id="p-tz-hint" class="hint">Your days start at midnight here.</p>
				</div>

				<div class="units">
					<fieldset class="field">
						<legend class="label">Weight unit</legend>
						<div class="segmented">
							{#each ['kg', 'lb'] as const as u (u)}
								<label
									><input type="radio" name="weight-unit" value={u} checked={f.weight_unit === u}
										onchange={() => (f = setWeightUnit(f!, u))} />{u}</label
								>
							{/each}
						</div>
					</fieldset>
					<fieldset class="field">
						<legend class="label">Height unit</legend>
						<div class="segmented">
							{#each [['cm', 'cm'], ['ftin', 'ft/in']] as const as [u, label] (u)}
								<label
									><input type="radio" name="height-unit" value={u} checked={f.height_unit === u}
										onchange={() => (f = setHeightUnit(f!, u))} />{label}</label
								>
							{/each}
						</div>
					</fieldset>
				</div>

				<fieldset class="field">
					<legend class="label">Formula: male / female (Mifflin–St Jeor)</legend>
					<div class="segmented" aria-describedby={describe('sex', 'p-sex-hint')}>
						<label><input type="radio" name="sex" value="male" bind:group={f.sex} />Male</label>
						<label><input type="radio" name="sex" value="female" bind:group={f.sex} />Female</label>
					</div>
					<p id="p-sex-hint" class="hint">Sets the calorie formula and the minimum floor (1,500 or 1,200 kcal).</p>
					{#if errors.sex}<p id="sex-err" class="err">{errors.sex}</p>{/if}
				</fieldset>

				<div class="field">
					<label for="p-year">Birth year</label>
					<input id="p-year" type="number" inputmode="numeric" min="1900" max={thisYear} bind:value={f.birth_year}
						aria-invalid={invalid('birth_year')} aria-describedby={describe('birth_year')} />
					{#if errors.birth_year}<p id="birth_year-err" class="err">{errors.birth_year}</p>{/if}
				</div>

				{#if f.height_unit === 'cm'}
					<div class="field">
						<label for="p-cm">Height (cm)</label>
						<input id="p-cm" type="number" inputmode="decimal" step="any" bind:value={f.height_cm}
							aria-invalid={invalid('height')} aria-describedby={describe('height')} />
						{#if errors.height}<p id="height-err" class="err">{errors.height}</p>{/if}
					</div>
				{:else}
					<fieldset class="field">
						<legend class="label">Height</legend>
						<div class="pair">
							<label class="unit-input"
								><input type="number" inputmode="numeric" min="1" max="8" aria-label="Height, feet" bind:value={f.height_ft}
									aria-invalid={invalid('height')} aria-describedby={describe('height')} /><span aria-hidden="true">ft</span></label
							>
							<label class="unit-input"
								><input type="number" inputmode="decimal" step="any" min="0" max="11.9" aria-label="Height, inches" bind:value={f.height_in}
									aria-invalid={invalid('height')} aria-describedby={describe('height')} /><span aria-hidden="true">in</span></label
							>
						</div>
						{#if errors.height}<p id="height-err" class="err">{errors.height}</p>{/if}
					</fieldset>
				{/if}

				<div class="field">
					<label for="p-start">Start weight ({f.weight_unit})</label>
					<input id="p-start" type="number" inputmode="decimal" step="any" bind:value={f.start_weight}
						aria-invalid={invalid('start_weight')} aria-describedby={describe('start_weight', 'p-start-hint')} />
					<p id="p-start-hint" class="hint">Used until you log a weigh-in.</p>
					{#if errors.start_weight}<p id="start_weight-err" class="err">{errors.start_weight}</p>{/if}
				</div>

				<div class="field">
					<label for="p-goal">Goal weight ({f.weight_unit}, optional)</label>
					<input id="p-goal" type="number" inputmode="decimal" step="any" bind:value={f.goal_weight}
						aria-invalid={invalid('goal_weight')} aria-describedby={describe('goal_weight', 'p-goal-hint')} />
					<p id="p-goal-hint" class="hint">Sets your protein target. Without it, your current weight does.</p>
					{#if errors.goal_weight}<p id="goal_weight-err" class="err">{errors.goal_weight}</p>{/if}
				</div>

				<div class="field">
					<label for="p-pace">Pace ({f.weight_unit} per week)</label>
					<input id="p-pace" type="number" inputmode="decimal" min="0" max={f.weight_unit === 'kg' ? 1 : 2.2}
						step={f.weight_unit === 'kg' ? 0.05 : 0.1} bind:value={f.pace}
						aria-invalid={invalid('pace')} aria-describedby={describe('pace', 'p-pace-hint')} />
					<p id="p-pace-hint" class="hint">How fast to lose weight; 0 to maintain. Up to {f.weight_unit === 'kg' ? '1 kg' : '2.2 lb'} a week.</p>
					{#if errors.pace}<p id="pace-err" class="err">{errors.pace}</p>{/if}
				</div>

				<div class="field">
					<label for="p-steps">Typical steps a day</label>
					<input id="p-steps" type="number" inputmode="numeric" min="0" max="150000" step="500" bind:value={f.typical_steps}
						aria-invalid={invalid('typical_steps')} aria-describedby={describe('typical_steps', 'p-steps-hint')} />
					<p id="p-steps-hint" class="hint">Used for a day until its steps are logged.</p>
					{#if errors.typical_steps}<p id="typical_steps-err" class="err">{errors.typical_steps}</p>{/if}
				</div>
			</fieldset>

			<p class="hint">Changes apply from today; past days keep their old targets.</p>
			{#if saveError}<p class="notice notice-error" role="alert">{saveError}</p>{/if}
			<p class="status" role="status">{status}</p>
			<button class="btn btn-primary btn-block" disabled={!online || busy}>{busy ? 'Saving…' : 'Save profile'}</button>
		</form>
	{/if}
</section>

<style>
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
		min-width: 0;
	}
	legend.label {
		padding: 0;
		margin-bottom: var(--space-1);
	}
	.units,
	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3);
	}
	.unit-input {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.err {
		color: var(--error);
		font-size: var(--text-sm);
	}
	.status {
		color: var(--under);
		font-size: var(--text-sm);
	}
	.field-skel {
		height: var(--touch);
	}
</style>
