<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  WeighInCard — the day's weight, the change since the last weigh-in and a 30-day line. A day with
  none shows the last one carried forward. Props: ctl: ActivityDay.
-->
<script lang="ts">
	import { dayLabel } from '#lib/date.ts';
	import { today } from '#lib/session.svelte.ts';
	import type { ActivityDay } from './day.svelte.ts';
	import FigureField from './FigureField.svelte';
	import { parseWeight } from './logic.ts';

	let { ctl }: { ctl: ActivityDay } = $props();
	const d = $derived(ctl.data!);
</script>

<div class="card stat stat-weight">
	<div class="card-head">
		<h2 id="weight-h">Weigh-in</h2>
		<span class="chip">{d.weight_kg !== null ? 'Manual' : ctl.carried ? 'Carried' : 'Not logged'}</span>
	</div>
	<FigureField
		id="act-weight"
		label="Weigh-in ({ctl.unit})"
		unit={ctl.unit}
		mode="decimal"
		placeholder="0.0"
		bind:value={ctl.weight}
		disabled={ctl.future || ctl.loading}
		onchange={() => ctl.commit(d.weight_kg, parseWeight(ctl.weight, ctl.unit), 'weight_kg', 'Weigh-in saved.')}
	/>
	{#if ctl.delta}
		<p class="note">
			<span class="num">{ctl.signed(ctl.delta.deltaKg)}</span> since {dayLabel(ctl.delta.prevDate, today())}
		</p>
	{/if}
	{#if ctl.trend}
		<svg class="spark" viewBox="0 0 120 32" preserveAspectRatio="none" role="img" aria-label="Weigh-ins over the last 30 days">
			<polyline points={ctl.trend} />
		</svg>
	{/if}
	<p class="note small">
		{#if ctl.carried}Carried from {dayLabel(ctl.carried.date, today())}.{' '}{/if}Used from this day until your next weigh-in.
	</p>
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
	.note.small {
		font-size: var(--text-xs);
	}
	.note .num {
		color: var(--text);
		font-weight: var(--weight-bold);
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
</style>
