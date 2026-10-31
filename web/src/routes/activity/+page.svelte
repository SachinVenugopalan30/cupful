<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import DayNav from '#lib/DayNav.svelte';
	import ActivityBudget from '#lib/activity/ActivityBudget.svelte';
	import ActivitySection from '#lib/activity/ActivitySection.svelte';
	import ActivityWeek from '#lib/activity/ActivityWeek.svelte';
	import WorkoutsPanel from '#lib/activity/WorkoutsPanel.svelte';
	import type { ActivityWindow } from '#lib/activity/logic.ts';
	import type { Budget } from '#lib/api.ts';
	import BudgetCard from '#lib/today/BudgetCard.svelte';
	import { day } from '#lib/session.svelte.ts';

	let refreshKey = $state(0);
	const changed = () => refreshKey++;
	// ActivitySection fetches the days and workouts once; the week strip and workouts share them.
	let win = $state<ActivityWindow | null>(null);
	let budget = $state<Budget | null>(null);
</script>

<svelte:head><title>Activity · Cupful</title></svelte:head>

<div class="activity stack">
	<header class="page-head">
		<h1>Activity</h1>
		<DayNav />
	</header>
	<div class="panels">
		<div class="main">
			<ActivitySection date={day.date} {refreshKey} onChanged={changed} heading={false} bind:win />
			<ActivityWeek date={day.date} {win} />
			<WorkoutsPanel date={day.date} {win} />
		</div>
		<div class="side">
			<BudgetCard date={day.date} {refreshKey} bind:budget />
			<ActivityBudget {budget} />
		</div>
	</div>
</div>

<style>
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.panels {
		display: grid;
		gap: var(--space-5);
	}
	.main,
	.side {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	/* Wide: the budget sits beside the stat cards, so the page uses its full width. */
	@media (min-width: 68rem) {
		.panels {
			grid-template-columns: minmax(0, 1.8fr) minmax(21rem, 1fr);
			align-items: start;
		}
	}
</style>
