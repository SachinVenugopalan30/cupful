<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import DayNav from '#lib/DayNav.svelte';
	import BudgetCard from '#lib/today/BudgetCard.svelte';
	import Composer from '#lib/today/Composer.svelte';
	import ActivitySection from '#lib/activity/ActivitySection.svelte';
	import MealList from '#lib/today/MealList.svelte';
	import BarcodeEntry from '#lib/products/BarcodeEntry.svelte';
	import { day } from '#lib/session.svelte.ts';

	// One key for the whole day: any write bumps it, and the budget and meal list refetch.
	let refreshKey = $state(0);
	const changed = () => refreshKey++;
</script>

<svelte:head><title>Today · Cupful</title></svelte:head>

<div class="today">
	<div class="nav"><DayNav /></div>
	<div class="budget"><BudgetCard date={day.date} {refreshKey} /></div>
	<div class="log">
		<Composer date={day.date} onSaved={changed} />
		<BarcodeEntry date={day.date} onSaved={changed} />
	</div>
	<div class="meals"><MealList date={day.date} {refreshKey} onChanged={changed} /></div>
	<!-- Phones have the Activity tab instead. -->
	<div class="activity"><ActivitySection date={day.date} {refreshKey} onChanged={changed} /></div>
</div>

<style>
	.today {
		display: grid;
		gap: var(--space-5);
		grid-template-areas: 'nav' 'budget' 'log' 'meals';
		grid-template-columns: minmax(0, 1fr);
	}
	.nav {
		grid-area: nav;
	}
	.budget {
		grid-area: budget;
	}
	.log {
		grid-area: log;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.meals {
		grid-area: meals;
	}
	.activity {
		grid-area: activity;
		display: none;
	}
	/* Wide: the budget sits beside logging, then the meals and activity span the page. */
	@media (min-width: 60rem) {
		.today {
			grid-template-areas: 'nav nav' 'budget log' 'meals meals' 'activity activity';
			grid-template-columns: 22rem minmax(0, 1fr);
			grid-template-rows: auto auto auto auto;
			column-gap: var(--space-6);
		}
		.activity {
			display: block;
		}
		.budget {
			align-self: start;
		}
	}
</style>
