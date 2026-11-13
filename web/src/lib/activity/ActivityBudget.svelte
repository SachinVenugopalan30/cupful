<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivityBudget — what steps and gym added to the day's budget, from the server's budget read. The
  two parts are the server's numbers; the total only adds them for display. With a fixed daily goal
  (manual mode) activity does not move the budget, so the card says what was burned instead.
  Props: date: string, refreshKey: number (refetch when it changes).
-->
<script lang="ts">
	import { getBudget, type Budget } from '#lib/api.ts';
	import { formatKcal } from '#lib/format.ts';
	import { today } from '#lib/session.svelte.ts';
	import CountUp from '#lib/motion/CountUp.svelte';
	import { dataVersion } from '#lib/offline/state.svelte.ts';
	import { addedToBudget } from './logic.ts';

	let { date, refreshKey }: { date: string; refreshKey: number } = $props();

	let budget = $state<Budget | null>(null);
	let missing = $state(false); // no profile yet (409)
	$effect(() => {
		const d = date;
		void refreshKey;
		void dataVersion.n;
		let stale = false;
		getBudget(d).then(
			(b) => !stale && ((budget = b), (missing = false)),
			() => !stale && ((budget = null), (missing = true))
		);
		return () => (stale = true);
	});

	const parts = $derived(budget ? addedToBudget(budget) : null);
	const heading = $derived(
		budget?.manual ? 'Burned beyond base' : date === today() ? "Added to today's budget" : "Added to this day's budget"
	);
</script>

<section class="card added" aria-label="Budget" aria-busy={budget === null && !missing} data-testid="activity-budget">
	<div class="card-head"><h2>{parts ? heading : "Added to today's budget"}</h2></div>
	{#if parts && budget}
		<p class="fig"><CountUp value={parts.total} prefix="+" /><small> kcal</small></p>
		<ul class="rows rise">
			<li style="--n: 1"><span class="grow">Steps beyond your base</span><span class="val pos num">+{formatKcal(parts.steps)}</span></li>
			<li style="--n: 2"><span class="grow">Gym</span><span class="val pos num">+{formatKcal(parts.gym)}</span></li>
		</ul>
		<p class="note">
			{#if budget.manual}
				Goal <span class="num">{formatKcal(budget.budget)}</span> kcal every day.
			{:else}
				<span class="num">{formatKcal(budget.budget)}</span> kcal budget for the day.
			{/if}
		</p>
	{:else if missing}
		<p class="note">Save your profile to see what activity adds. <a href="/profile">Open Profile</a></p>
	{:else}
		<div class="skeleton skel" aria-hidden="true"></div>
	{/if}
</section>

<style>
	.added {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.fig {
		color: var(--under);
	}
	.pos {
		color: var(--under);
	}
	.note {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.note .num {
		color: var(--text);
		font-weight: var(--weight-bold);
	}
	.skel {
		height: 5rem;
		border-radius: var(--radius-md);
	}
</style>
