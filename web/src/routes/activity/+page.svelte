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
	<DayNav />
	<ActivitySection date={day.date} {refreshKey} onChanged={changed} />
	<BudgetCard date={day.date} {refreshKey} />
</div>

<style>
	.activity {
		max-width: var(--content-max);
		margin: 0 auto;
	}
</style>
