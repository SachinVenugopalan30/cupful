<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  GymCard — gym minutes and the body parts trained. The parts show as chips; the chips are also the
  "Trained" menu button, where a part saves as soon as it is ticked. Props: ctl: ActivityDay.
-->
<script lang="ts">
	import { dayLabel } from '#lib/date.ts';
	import { dismiss } from '#lib/dismiss.ts';
	import { today } from '#lib/session.svelte.ts';
	import type { ActivityDay } from './day.svelte.ts';
	import FigureField from './FigureField.svelte';
	import { BODY_PARTS, MAX_GYM, parseCount, sessionsIn, sourceLabel } from './logic.ts';

	let { ctl }: { ctl: ActivityDay } = $props();
	const d = $derived(ctl.data!);
	const n = $derived(ctl.sessions ? sessionsIn(ctl.sessions.gym, ctl.weekDates) : 0);
</script>

<div class="card stat stat-gym">
	<div class="card-head">
		<h2 id="gym-h">Gym</h2>
		<span class="chip">{d.gym_minutes === null ? 'Not logged' : d.gym_source ? sourceLabel(d.gym_source) : 'Manual'}</span>
	</div>
	<FigureField
		id="act-gym"
		label="Gym minutes"
		unit="min"
		bind:value={ctl.gym}
		disabled={ctl.future || ctl.loading}
		onchange={() => ctl.commit(d.gym_minutes, parseCount(ctl.gym, MAX_GYM, 'Gym minutes'), 'gym_minutes', 'Gym minutes saved.')}
	/>
	<details class="menu trained" use:dismiss>
		<summary
			class="btn trained-btn"
			aria-label="Trained body parts: {ctl.trained.length ? ctl.trained.join(', ') : 'none'}"
		>
			<span class="trained-label">Trained</span>
			{#each ctl.trained as part (part)}<span class="chip chip-accent pop">{part}</span>{:else}<span class="none">none</span>{/each}
		</summary>
		<div class="menu-panel">
			<fieldset class="parts">
				<legend class="visually-hidden">Trained body parts</legend>
				{#each BODY_PARTS as part (part)}
					<label class="part">
						<input
							type="checkbox"
							checked={ctl.trained.includes(part)}
							disabled={ctl.future || ctl.loading}
							onchange={() => ctl.togglePart(part)}
						/>
						{part}
					</label>
				{/each}
			</fieldset>
			<button type="button" class="btn done" data-close>Done</button>
		</div>
	</details>
	{#if ctl.sessions}
		<p class="note">
			<span class="num">{n}</span>
			{n === 1 ? 'session' : 'sessions'} in the last 7 days{#if ctl.sessions.last}; last trained {dayLabel(ctl.sessions.last, today())}{/if}
		</p>
	{/if}
</div>

<style>
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
	.note {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.note .num {
		color: var(--text);
		font-weight: var(--weight-bold);
	}
	.trained-btn {
		justify-content: flex-start;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-2);
		width: 100%;
		min-width: 0;
		font-size: var(--text-sm);
	}
	.trained-label,
	.none {
		color: var(--text-muted);
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
		transition: transform var(--dur-med) var(--ease);
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
</style>
