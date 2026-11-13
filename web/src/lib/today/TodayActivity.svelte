<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  TodayActivity — two tiles under the jug: Steps (against typical steps; Cupful has no step goal)
  and Gym (minutes and trained parts). Read from the day read; each tile opens /activity.
  Hidden until the day has steps or gym minutes, so people who track neither see no empty tiles.
  Props:
    date: string        YYYY-MM-DD, the selected day
    refreshKey: number  bumped after a write; refetch when it changes
-->
<script lang="ts">
	import { getDay, type Day } from '#lib/api.ts';
	import CountUp from '#lib/motion/CountUp.svelte';
	import { session } from '#lib/session.svelte.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import { capitalize } from './meal.ts';
	import { stepsTile } from './week.ts';

	let { date, refreshKey }: { date: string; refreshKey: number } = $props();

	let d = $state<Day | null>(null);

	$effect(() => {
		const shown = date;
		void refreshKey;
		void dataVersion.n;
		if (!shown) return;
		let stale = false;
		getDay(shown).then(
			(x) => !stale && (d = x),
			// Tiles are a convenience; the Activity page has the full story and the errors.
			() => !stale && (d = null)
		);
		return () => (stale = true);
	});

	const typical = $derived(session.profile?.version?.typical_steps ?? 0);
	const steps = $derived(d?.steps ?? null);
	const gym = $derived(d?.gym_minutes ?? null);
	const vs = $derived(steps === null ? null : stepsTile(steps, typical));
	const trained = $derived((d?.trained ?? []).map(capitalize).join(', '));
</script>

{#if d && (steps !== null || gym !== null)}
	<section class="tiles" aria-label="Activity">
		<a class="tile steps rise" href="/activity" style="--n: 0">
			<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 20h5v-5h5v-5h5V5h3" /></svg>
			<p class="fig">
				{#if steps !== null}<CountUp value={steps} />{:else}<span class="num">–</span>{/if}
			</p>
			<p class="note">
				{#if steps === null}no steps yet
				{:else if vs && typical > 0}steps {vs.label}
				{:else}steps{/if}
			</p>
			{#if vs && typical > 0}
				<div
					class="meter"
					style="--value: {vs.value}; --fill: var(--steps)"
					role="meter"
					aria-label="Steps against typical"
					aria-valuemin="0"
					aria-valuemax={typical}
					aria-valuenow={steps ?? 0}
					aria-valuetext="{(steps ?? 0).toLocaleString('en-US')} {vs.label}"
				></div>
			{/if}
		</a>
		<a class="tile gym rise" href="/activity" style="--n: 1">
			<svg viewBox="0 0 24 24" aria-hidden="true"
				><path d="M6 8v8M18 8v8M3 10v4M21 10v4M6 12h12" /></svg
			>
			<p class="fig">
				{#if gym !== null}<CountUp value={gym} /><small> min</small>{:else}<span class="num">–</span>{/if}
			</p>
			<p class="note">{gym === null ? 'no gym yet' : trained ? `gym · ${trained}` : 'gym'}</p>
		</a>
	</section>
{/if}

<style>
	.tiles {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3);
	}
	.tile {
		padding: var(--space-4);
		gap: 0.125rem;
	}
	.tile .fig {
		font-size: 1.75rem;
		line-height: 1.1;
	}
	.note {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}
	svg {
		inline-size: 1.25rem;
		block-size: 1.25rem;
		margin-block-end: 0.375rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.steps svg {
		color: var(--steps);
	}
	.gym svg {
		color: var(--protein);
	}
	.meter {
		margin-block-start: var(--space-2);
		height: 0.375rem;
	}
	.tile:focus-visible {
		outline: 3px solid var(--focus-ring);
		outline-offset: 2px;
	}
</style>
