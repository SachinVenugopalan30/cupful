<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import DayNav from '#lib/DayNav.svelte';
	import BudgetCard from '#lib/today/BudgetCard.svelte';
	import Composer from '#lib/today/Composer.svelte';
	import UsualMeal from '#lib/today/UsualMeal.svelte';
	import TodayActivity from '#lib/today/TodayActivity.svelte';
	import TodayWeek from '#lib/today/TodayWeek.svelte';
	import MealList from '#lib/today/MealList.svelte';
	import BarcodeEntry from '#lib/products/BarcodeEntry.svelte';
	import type { Budget } from '#lib/api.ts';
	import { day } from '#lib/session.svelte.ts';

	// One key for the whole day: any write bumps it, and the budget and meal list refetch.
	let refreshKey = $state(0);
	const changed = () => refreshKey++;
	// The budget card reports what it loaded; the week card follows it.
	let budget = $state<Budget | null>(null);
	let eaten = $state(0);
</script>

<svelte:head><title>Today · Cupful</title></svelte:head>

<div class="today">
	<header class="page-head">
		<h1>Today</h1>
		<DayNav />
	</header>
	<!-- Phones stack the two columns: jug and activity, then logging and meals. -->
	<div class="col">
		<BudgetCard date={day.date} {refreshKey} bind:budget bind:eaten />
		<TodayActivity date={day.date} {refreshKey} />
		<TodayWeek date={day.date} {refreshKey} plan={budget?.budget ?? null} {eaten} />
	</div>
	<div class="col">
		<div class="log">
			{#key day.date}
				<UsualMeal date={day.date} {refreshKey} onSaved={changed} />
			{/key}
			<Composer date={day.date} onSaved={changed} />
			<BarcodeEntry date={day.date} onSaved={changed} />
		</div>
		<MealList date={day.date} {refreshKey} onChanged={changed} />
	</div>
</div>

<style>
	.today {
		display: grid;
		gap: var(--space-5);
		grid-template-columns: minmax(0, 1fr);
	}
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.col {
		display: grid;
		gap: var(--space-5);
		grid-template-columns: minmax(0, 1fr);
		align-content: start;
		min-width: 0;
	}
	.log {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	/* Wide: a 400px column for the jug, activity and week beside the flexible logging column. */
	@media (min-width: 60rem) {
		.today {
			grid-template-columns: 25rem minmax(0, 1fr);
			column-gap: var(--space-5);
			align-items: start;
		}
		.page-head {
			grid-column: 1 / -1;
		}
	}
</style>
