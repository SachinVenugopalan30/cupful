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
	{#if !isToday}
		<button class="btn btn-quiet back" onclick={() => (day.date = now)} aria-label="Back to today">
			<span class="back-long">Back to today</span>
			<span class="back-short" aria-hidden="true">Today</span>
		</button>
	{/if}
	<button class="btn btn-quiet step" onclick={() => (day.date = addDays(day.date, -1))} aria-label="Previous day">
		<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>
	</button>
	<div class="current">
		<label class="visually-hidden" for="day-picker">Date</label>
		<input id="day-picker" type="date" value={day.date} max={now} onchange={pick} />
	</div>
	<button
		class="btn btn-quiet step"
		onclick={() => (day.date = addDays(day.date, 1))}
		disabled={isToday}
		aria-label="Next day"
	>
		<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5l7 7-7 7" /></svg>
	</button>
	<p class="visually-hidden" aria-live="polite">{dayLabel(day.date, now)}</p>
</nav>

<style>
	.daynav {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
		flex-wrap: wrap;
		margin-left: auto;
	}
	.current {
		display: flex;
		align-items: center;
		min-width: 0;
	}
	.step {
		width: var(--touch);
		padding: 0;
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
		flex: none;
	}
	.back-short {
		display: none;
	}
	@media (max-width: 47.99rem) {
		.daynav {
			gap: var(--space-1);
		}
		input {
			width: 8.5rem;
		}
		.back {
			padding-inline: var(--space-2);
		}
		.back-long {
			display: none;
		}
		.back-short {
			display: inline;
		}
	}
</style>
