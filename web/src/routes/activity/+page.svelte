<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import DayNav from '#lib/DayNav.svelte';
	import ActivityBudget from '#lib/activity/ActivityBudget.svelte';
	import { ActivityDay } from '#lib/activity/day.svelte.ts';
	import ActivityWeek from '#lib/activity/ActivityWeek.svelte';
	import GymCard from '#lib/activity/GymCard.svelte';
	import { weekEndFor } from '#lib/activity/logic.ts';
	import StepsCard from '#lib/activity/StepsCard.svelte';
	import WeighInCard from '#lib/activity/WeighInCard.svelte';
	import WorkoutsPanel from '#lib/activity/WorkoutsPanel.svelte';
	import { day } from '#lib/session.svelte.ts';

	let refreshKey = $state(0);
	// The seven-day graph stays put while the selected day is inside it.
	let weekEnd = $state(day.date);
	$effect(() => {
		weekEnd = weekEndFor(day.date, weekEnd);
	});

	const ctl = new ActivityDay(() => ({
		date: day.date,
		refreshKey,
		windowEnd: weekEnd,
		syncSteps: true,
		onChanged: () => refreshKey++
	}));
</script>

<svelte:head><title>Activity · Cupful</title></svelte:head>

<div class="activity stack">
	<header class="page-head">
		<h1>Activity</h1>
		<DayNav />
	</header>
	<p class="saved" role="status">{ctl.status}</p>

	<section class="stack" aria-label="Activity" aria-busy={ctl.loading || (ctl.data === null && !ctl.loadError)}>
		{#if ctl.future}<p class="notice notice-info">Future days can't be logged yet.</p>{/if}
		{#if ctl.error}<p class="notice notice-error" role="alert">{ctl.error}</p>{/if}
		{#if ctl.loadError}
			<div class="notice notice-error" role="alert">
				<p>{ctl.loadError}</p>
				<div class="row"><button class="btn" onclick={() => ctl.retry++}>Try again</button></div>
			</div>
		{/if}

		<div class="grid">
		<div class="budget"><ActivityBudget date={day.date} {refreshKey} /></div>
		{#if ctl.data}
			<div class="steps"><StepsCard {ctl} quiet /></div>
			<div class="gym"><GymCard {ctl} /></div>
		{:else if !ctl.loadError}
			<div class="steps skeleton skel" aria-hidden="true"></div>
			<div class="gym skeleton skel" aria-hidden="true"></div>
		{/if}
		<div class="week"><ActivityWeek date={day.date} end={weekEnd} win={ctl.win} /></div>
		<div class="workouts"><WorkoutsPanel date={day.date} win={ctl.win} /></div>
		{#if ctl.data}<div class="weigh"><WeighInCard {ctl} /></div>{/if}
		</div>
	</section>
</div>

<style>
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.saved {
		min-height: 1.25rem;
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.skel {
		min-height: 10rem;
		border-radius: var(--radius-lg);
	}
	/* Phone: one column, with Steps and Gym side by side once there is room for two. */
	.grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-3);
		align-items: start;
	}
	.grid > * {
		min-width: 0;
	}
	@media (min-width: 30rem) {
		.grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.budget,
		.week,
		.workouts,
		.weigh {
			grid-column: 1 / -1;
		}
	}
	/* Desktop: budget, steps, gym; then the week across two columns beside weigh-in and workouts. */
	@media (min-width: 60rem) {
		.grid {
			grid-template-columns: repeat(3, minmax(0, 1fr));
			grid-template-areas:
				'budget steps gym'
				'week week weigh'
				'week week workouts';
			gap: var(--space-5);
		}
		.budget {
			grid-area: budget;
		}
		.steps {
			grid-area: steps;
		}
		.gym {
			grid-area: gym;
		}
		.week {
			grid-area: week;
		}
		.weigh {
			grid-area: weigh;
		}
		.workouts {
			grid-area: workouts;
		}
	}
</style>
