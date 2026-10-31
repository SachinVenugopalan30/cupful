<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import DayNav from '#lib/DayNav.svelte';
	import ActivitySection from '#lib/activity/ActivitySection.svelte';
	import BudgetCard from '#lib/today/BudgetCard.svelte';
	import { day } from '#lib/session.svelte.ts';

	let refreshKey = $state(0);
	const changed = () => refreshKey++;
</script>

<svelte:head><title>Activity · Cupful</title></svelte:head>

<div class="activity stack">
	<header class="page-head">
		<h1>Activity</h1>
		<DayNav />
	</header>
	<div class="panels">
		<ActivitySection date={day.date} {refreshKey} onChanged={changed} heading={false} />
		<BudgetCard date={day.date} {refreshKey} />
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
	/* Wide: the budget sits beside the stat cards, so the page uses its full width. */
	@media (min-width: 68rem) {
		.panels {
			grid-template-columns: minmax(0, 1.8fr) minmax(21rem, 1fr);
			align-items: start;
		}
	}
</style>
