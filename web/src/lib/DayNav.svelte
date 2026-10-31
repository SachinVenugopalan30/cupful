<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Day navigation: previous / next / native date picker. Reads and writes `day.date` from
  #lib/session.svelte. Future dates can't be picked; "today" is the user's, in their timezone.
-->
<script lang="ts">
	import { addDays, dayLabel } from '#lib/date.ts';
	import { day, today } from '#lib/session.svelte.ts';

	const now = $derived(today());
	const isToday = $derived(day.date >= now);

	function pick(e: Event) {
		const v = (e.currentTarget as HTMLInputElement).value;
		// Typed dates can still be past `max`; empty means the picker was cleared.
		if (v) day.date = v > now ? now : v;
	}
</script>

<nav class="daynav" aria-label="Day">
	<button class="btn btn-quiet" onclick={() => (day.date = addDays(day.date, -1))} aria-label="Previous day">
		<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>
	</button>
	<div class="current">
		<label class="visually-hidden" for="day-picker">Date</label>
		<input id="day-picker" type="date" value={day.date} max={now} onchange={pick} />
	</div>
	<button
		class="btn btn-quiet"
		onclick={() => (day.date = addDays(day.date, 1))}
		disabled={isToday}
		aria-label="Next day"
	>
		<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5l7 7-7 7" /></svg>
	</button>
	<p class="visually-hidden" aria-live="polite">{dayLabel(day.date, now)}</p>
</nav>
{#if !isToday}
	<p class="back"><button class="btn btn-quiet" onclick={() => (day.date = now)}>Back to today</button></p>
{/if}

<style>
	.daynav {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.current {
		display: flex;
		align-items: center;
		min-width: 0;
	}
	input {
		width: auto;
		padding: 0 var(--space-3);
		border-color: transparent;
		background: var(--surface-2);
		color: var(--text-muted);
	}
	svg {
		width: 1.5rem;
		height: 1.5rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.back {
		text-align: right;
		margin-top: calc(-1 * var(--space-2));
	}
</style>
