<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ActivityBudget — one line: what steps and gym added to the day's budget. The numbers are the
  server's (BudgetCard's "How it's worked out"); nothing is recomputed here.
  Props: budget: Budget | null   from BudgetCard's bindable `budget`; null shows nothing
-->
<script lang="ts">
	import type { Budget } from '#lib/api.ts';
	import { formatKcal } from '#lib/format.ts';

	let { budget }: { budget: Budget | null } = $props();
</script>

{#if budget}
	<p class="added card" data-testid="activity-budget">
		<span class="lead">Added to this day's budget</span>
		<span class="terms">
			<span>Steps <span class="num">+{formatKcal(budget.steps_kcal)}</span> kcal</span>
			<span aria-hidden="true">·</span>
			<span>Gym <span class="num">+{formatKcal(budget.gym_kcal)}</span> kcal</span>
			<span aria-hidden="true">→</span>
			<span><span class="num strong">{formatKcal(budget.budget)}</span> kcal budget</span>
		</span>
	</p>
{/if}

<style>
	.added {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		font-size: var(--text-sm);
		min-width: 0;
	}
	.lead {
		color: var(--text-muted);
	}
	.terms {
		display: flex;
		flex-wrap: wrap;
		gap: 0 var(--space-2);
	}
	.strong {
		font-weight: var(--weight-bold);
	}
</style>
