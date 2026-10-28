<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Composer — meal type, description, optional photo, Estimate → review → save; manual entry.
  Props (fixed contract; Today page passes these):
    date: string          YYYY-MM-DD the meal is logged on
    onSaved: () => void   call after sendOps reports the meal applied
  Writes only through #lib/sync/index.ts (mealUpsert + sendOps); scaleItems before saving. AI errors:
  errorMessage(e, 'estimate') from #lib/api. Photos: re-encode through a canvas before upload.
-->
<script lang="ts">
	let { date }: { date: string; onSaved: () => void } = $props();
</script>

<section class="stub" aria-label="Log a meal">
	<h2>Log a meal</h2>
	<p>Composer placeholder · {date}</p>
</section>
