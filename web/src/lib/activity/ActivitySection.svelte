<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivitySection — the steps, gym and weigh-in cards for one day as a self-contained block with its
  own saved line. The Activity page lays the same cards out itself (see routes/activity); this
  wrapper serves any screen that wants the three cards without that grid.
  Props: date, refreshKey, onChanged (as ActivityDay), syncSteps? (Google Health button).
-->
<script lang="ts">
	import { ActivityDay } from './day.svelte.ts';
	import GymCard from './GymCard.svelte';
	import StepsCard from './StepsCard.svelte';
	import WeighInCard from './WeighInCard.svelte';

	let {
		date,
		refreshKey,
		onChanged,
		syncSteps = false
	}: { date: string; refreshKey: number; onChanged: () => void; syncSteps?: boolean } = $props();

	const ctl = new ActivityDay(() => ({ date, refreshKey, syncSteps, onChanged }));
</script>

<section class="activity" aria-label="Activity" aria-busy={ctl.loading || (ctl.data === null && !ctl.loadError)}>
	<header class="card-head">
		<h2>Activity</h2>
		<p class="saved" role="status">{ctl.status}</p>
	</header>
	{#if ctl.loadError}
		<div class="notice notice-error" role="alert">
			<p>{ctl.loadError}</p>
			<div class="row"><button class="btn" onclick={() => ctl.retry++}>Try again</button></div>
		</div>
	{:else if ctl.data === null}
		<div class="stats" aria-hidden="true">
			<div class="skeleton skeleton-card"></div>
			<div class="skeleton skeleton-card"></div>
			<div class="skeleton skeleton-card"></div>
		</div>
	{:else}
		{#if ctl.future}<p class="notice notice-info">Future days can't be logged yet.</p>{/if}
		{#if ctl.error}<p class="notice notice-error" role="alert">{ctl.error}</p>{/if}
		<div class="stats">
			<StepsCard {ctl} />
			<GymCard {ctl} />
			<WeighInCard {ctl} />
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
	.saved {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
		gap: var(--space-3);
	}
	.skeleton-card {
		min-height: 10rem;
		border-radius: var(--radius-lg);
	}
</style>
