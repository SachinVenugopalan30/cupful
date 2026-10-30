<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { errorMessage, getTrends, type TrendDay } from '#lib/api.ts';
	import { session, today } from '#lib/session.svelte.ts';
	import { dataVersion } from '#lib/offline/state.svelte.ts';
	import { averages, loadRange, rangeDates, RANGES, saveRange, type Range } from '#lib/trends/logic.ts';

	let range = $state<Range>(loadRange());
	let days = $state<TrendDay[] | null>(null);
	let error = $state('');
	let retry = $state(0);

	$effect(() => {
		saveRange(range);
		void dataVersion.n;
		void retry;
		const { from, to } = rangeDates(range, today());
		let stale = false;
		days = null;
		error = '';
		getTrends(from, to).then(
			(r) => !stale && (days = r),
			(e) => !stale && (error = errorMessage(e))
		);
		return () => (stale = true);
	});

	// The chart library loads only when this screen is opened.
	const charts = import('#lib/trends/Charts.svelte');
	const avg = $derived(days ? averages(days) : null);
	const gap = $derived.by(() => {
		if (avg?.intake == null || avg.budget === null) return '';
		const left = Math.round(avg.budget) - Math.round(avg.intake);
		return left >= 0 ? `${left} left` : `${-left} over`;
	});
	const kcal = (n: number | null) => (n === null ? '–' : String(Math.round(n)));
</script>

<svelte:head><title>Trends · Cupful</title></svelte:head>

<div class="trends stack">
	<h1>Trends</h1>

	<fieldset class="segmented">
		<legend class="visually-hidden">Range</legend>
		{#each RANGES as r (r)}
			<label>
				<input type="radio" name="range" value={r} bind:group={range} />
				<span aria-hidden="true">{r}d</span><span class="visually-hidden">{r} days</span>
			</label>
		{/each}
	</fieldset>

	{#if error}
		<div class="notice notice-error" role="alert">
			<p>{error}</p>
			<div class="row"><button class="btn" onclick={() => retry++}>Try again</button></div>
		</div>
	{:else if days === null}
		<div aria-busy="true" class="stack">
			<div class="skeleton" style="height: 5rem"></div>
			<div class="skeleton" style="height: 16rem"></div>
		</div>
	{:else}
		<section class="card summary" aria-label="Average intake against budget">
			{#if avg && avg.days}
				<p class="small muted">
					Average a day, over {avg.days} {avg.days === 1 ? 'day' : 'days'} with meals
				</p>
				<p class="big num">
					<span class="v">{kcal(avg.intake)}</span>
					<span class="of">kcal eaten</span>
				</p>
				<p class="num">
					{#if avg.budget === null}
						No budget yet for these days.
					{:else}
						Budget {kcal(avg.budget)} kcal, {gap} a day
					{/if}
				</p>
			{:else}
				<p class="muted">Averages appear once you've logged a meal in this range.</p>
			{/if}
		</section>
		{#await charts}
			<div class="skeleton" style="height: 16rem" aria-busy="true"></div>
		{:then { default: Charts }}
			<Charts {days} today={today()} unit={session.profile?.weight_unit ?? 'kg'} />
		{:catch}
			<p class="notice notice-error" role="alert">The charts didn't load. Reload the page to try again.</p>
		{/await}
	{/if}
</div>

<style>
	.trends {
		max-width: var(--content-max);
		margin: 0 auto;
	}
	.big {
		margin: 0;
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
	.v {
		font-size: var(--text-2xl);
		font-weight: var(--weight-bold);
	}
	.of {
		font-size: var(--text-md);
		color: var(--text-muted);
	}
</style>
