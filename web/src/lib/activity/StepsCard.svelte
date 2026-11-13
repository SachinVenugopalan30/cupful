<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  StepsCard — the day's steps, typed by hand or synced, against your typical day (there is no step
  goal). Props: ctl: ActivityDay (the loaded day, and the save), sync?: show the Google Health
  "Sync steps" button when connected, quiet?: keep the sync note out of the card (the page's
  status line already says it).
-->
<script lang="ts">
	import type { ActivityDay } from './day.svelte.ts';
	import FigureField from './FigureField.svelte';
	import { MAX_STEPS, parseCount, sourceLabel } from './logic.ts';

	let { ctl, quiet = false }: { ctl: ActivityDay; quiet?: boolean } = $props();
	const d = $derived(ctl.data!);
	const typical = $derived(ctl.typicalSteps);
</script>

<div class="card stat stat-steps">
	<div class="card-head">
		<h2 id="steps-h">Steps</h2>
		<span class="chip" data-testid="steps-source">
			{d.steps === null ? 'Not logged' : d.steps_source ? sourceLabel(d.steps_source) : 'Manual'}
		</span>
	</div>
	<FigureField
		id="act-steps"
		label="Steps"
		unit="steps"
		bind:value={ctl.steps}
		disabled={ctl.future || ctl.loading}
		onchange={() => ctl.commit(d.steps, parseCount(ctl.steps, MAX_STEPS, 'Steps'), 'steps', 'Steps saved.')}
	/>
	{#if typical !== null}
		<p class="note">of <span class="num">{typical.toLocaleString()}</span> typical</p>
	{:else}
		<p class="note">Steps for this day.</p>
	{/if}
	{#if ctl.pct !== null}
		<div
			class="meter"
			style="--value: {ctl.pct / 100}; --fill: var(--steps)"
			role="meter"
			aria-label="Steps against your typical day"
			aria-valuemin="0"
			aria-valuemax={typical}
			aria-valuenow={d.steps}
			aria-valuetext="{ctl.pct}% of your typical day"
		></div>
	{/if}
	{#if ctl.avg !== null}
		<p class="note">7-day average <span class="num">{Math.round(ctl.avg).toLocaleString()}</span></p>
	{/if}
	{#if ctl.canSync}
		<div class="sync-row">
			<button type="button" class="btn btn-soft" onclick={() => ctl.syncFromGoogle()} disabled={ctl.syncing || ctl.loading}>
				{ctl.syncing ? 'Syncing…' : 'Sync steps'}
			</button>
			{#if !quiet && ctl.syncNote}<span class="note">{ctl.syncNote}</span>{/if}
		</div>
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
	@media (prefers-reduced-motion: no-preference) {
		.stat :global(.meter)::after {
			animation: grow-x var(--dur-slow) var(--ease) both;
		}
	}
	.sync-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
</style>
