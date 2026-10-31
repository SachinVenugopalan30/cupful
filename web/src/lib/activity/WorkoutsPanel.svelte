<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  WorkoutsPanel — the workouts synced for the selected day (from Google Health). Read-only.
  Props:
    date: string                 the selected day
    win: ActivityWindow | null   from ActivitySection's `win`; null shows a skeleton
  Distances are in km: Cupful has no distance unit setting.
-->
<script lang="ts">
	import { google, type GoogleStatus } from '#lib/api.ts';
	import { session } from '#lib/session.svelte.ts';
	import { formatClock, formatDuration, formatKm, workoutKind, type ActivityWindow } from './logic.ts';

	let { date, win }: { date: string; win: ActivityWindow | null } = $props();

	const tz = $derived(session.profile?.timezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone);
	const list = $derived(win && win.date === date ? win.workouts.filter((w) => w.date === date) : null);

	// Only to decide whether to point at Settings; offline or unconfigured shows nothing extra.
	let status = $state<GoogleStatus | null>(null);
	$effect(() => {
		google.status().then(
			(s) => (status = s),
			() => (status = null)
		);
	});
	const notConnected = $derived(status?.configured === true && !status.connected);

	const PATHS: Record<string, string> = {
		run: 'M13 4a1.5 1.5 0 100 .01M9 20l2.5-5-3-2 2-5 4 2 2 3M7 11l3-3',
		walk: 'M13 4a1.5 1.5 0 100 .01M10 21l1.5-6-2-2 1.5-5 3 2 2 2M9 9l-2 3',
		bike: 'M6 18a3 3 0 100-.01M18 18a3 3 0 100-.01M6 18l4-8h5l3 8M10 10l-1-3h3',
		swim: 'M3 17c2 0 2-1.5 4.5-1.5S10 17 12 17s2.5-1.5 4.5-1.5S19 17 21 17M3 21c2 0 2-1.5 4.5-1.5S10 21 12 21M7 12l4-3 3 2',
		strength: 'M3 10v4M6 8v8M18 8v8M21 10v4M6 12h12',
		other: 'M3 12h4l2-6 4 12 2-6h6'
	};
</script>

<section class="card panel" aria-labelledby="workouts-h" aria-busy={list === null}>
	<header class="head">
		<h3 id="workouts-h">Workouts</h3>
		{#if list?.length}<p class="small muted">{list.length} synced</p>{/if}
	</header>

	{#if list === null}
		<div class="skeleton skel" aria-hidden="true"></div>
	{:else if list.length === 0}
		<div class="empty stack">
			<p>No workouts synced for this day.</p>
			{#if notConnected}
				<p class="small"><a href="/settings#google-h">Connect Google Health in Settings</a> to bring workouts in.</p>
			{/if}
		</div>
	{:else}
		<ul class="rows">
			{#each list as w (w.id)}
				<li class="row-w" data-testid="workout">
					<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={PATHS[workoutKind(w.type)]} /></svg>
					<div class="body">
						<p class="title">
							<span>{w.name || w.type_label}</span>
							{#if w.counts_as_gym}<span class="chip chip-gym">Counts as gym</span>{/if}
						</p>
						<p class="sub small muted">
							{w.name ? `${w.type_label} · ` : ''}{formatClock(w.start, tz)}
						</p>
						<p class="stats small">
							<span class="num">{formatDuration(w.active_minutes)}</span>
							{#if w.distance_m !== null}<span class="num">{formatKm(w.distance_m)}</span>{/if}
							{#if w.kcal !== null}<span class="num">{Math.round(w.kcal)} kcal</span>{/if}
							{#if w.avg_hr !== null}<span class="num">{w.avg_hr} bpm avg</span>{/if}
						</p>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.panel {
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
	}
	.head h3 {
		font-size: var(--text-lg);
		letter-spacing: -0.01em;
	}
	.skel {
		height: 4.5rem;
		border-radius: var(--radius-md);
	}
	.rows {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.row-w {
		display: flex;
		gap: var(--space-3);
		padding: var(--space-3) 0;
		border-top: 1px solid var(--border-strong);
	}
	.row-w:first-child {
		border-top: 0;
		padding-top: 0;
	}
	.icon {
		flex: none;
		width: 1.5rem;
		height: 1.5rem;
		margin-top: 0.125rem;
		fill: none;
		stroke: var(--steps);
		stroke-width: 1.75;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.body {
		min-width: 0;
		flex: 1;
	}
	.title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		font-weight: var(--weight-bold);
	}
	.chip-gym {
		background: var(--accent-soft);
		color: var(--accent);
		font-weight: var(--weight-medium);
	}
	.stats {
		display: flex;
		flex-wrap: wrap;
		gap: 0 var(--space-3);
		margin-top: var(--space-1);
	}
</style>
