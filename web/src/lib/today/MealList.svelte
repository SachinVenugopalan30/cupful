<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  MealList — logged meals grouped by meal type, edit, delete, "Eat this again".
  Props (fixed contract; Today page passes these):
    date: string             YYYY-MM-DD, the selected day
    refreshKey: number       bumped after any write to the day; refetch when it changes
    onChanged: () => void    call after an edit/delete/repeat is applied
  Data: getMeals(date). Writes only through #lib/sync/index.ts (mealUpsert / mealDelete + sendOps).
-->
<script lang="ts">
	let { date, refreshKey }: { date: string; refreshKey: number; onChanged: () => void } = $props();
</script>

<section class="stub" aria-label="Meals">
	<h2>Meals</h2>
	<p>MealList placeholder · {date} · refresh {refreshKey}</p>
</section>
