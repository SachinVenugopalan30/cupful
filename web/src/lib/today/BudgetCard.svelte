<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  BudgetCard — eaten vs budget, left/over, macro bars, flags, "how it's worked out".
  Props (fixed contract; Today page passes these):
    date: string        YYYY-MM-DD, the selected day
    refreshKey: number  bumped after any write to the day; refetch when it changes
  Data: getBudget(date) (409 until the profile is saved: link to Settings), getMeals(date) for
  eaten totals. Server computes the budget; never compute it here.
-->
<script lang="ts">
	let { date, refreshKey }: { date: string; refreshKey: number } = $props();
</script>

<section class="stub" aria-label="Budget">
	<h2>Budget</h2>
	<p>BudgetCard placeholder · {date} · refresh {refreshKey}</p>
</section>
