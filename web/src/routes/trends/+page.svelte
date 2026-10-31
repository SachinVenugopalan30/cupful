<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { errorMessage, getBudget, getTrends, type Budget, type Trends } from '#lib/api.ts';
	import { session, today } from '#lib/session.svelte.ts';
	import { dataVersion } from '#lib/offline/state.svelte.ts';
	import { averages, formatNumber, loadRange, RANGES, rangeDates, saveRange, type Range } from '#lib/trends/logic.ts';
	import Analytics from '#lib/trends/Analytics.svelte';
	import Charts from '#lib/trends/Charts.svelte';
	import Consistency from '#lib/trends/Consistency.svelte';
	import CountUp from '#lib/trends/CountUp.svelte';

	let range = $state<Range>(loadRange());
	let data = $state<Trends | null>(null);
	let targets = $state<Budget | null>(null);
	let error = $state('');
	let retry = $state(0);

	$effect(() => {
		saveRange(range);
		void dataVersion.n;
		void retry;
		const { from, to } = rangeDates(range, today());
		let stale = false;
		data = null;
		error = '';
		getTrends(from, to).then(
			(r) => !stale && (data = r),
			(e) => !stale && (error = errorMessage(e))
		);
		return () => (stale = true);
	});

	// Macro targets stay server-side: today's budget carries them.
	$effect(() => {
		const day = today();
		let stale = false;
		getBudget(day).then(
			(b) => !stale && (targets = b),
			() => !stale && (targets = null)
		);
		return () => (stale = true);
	});

	const avg = $derived(data ? averages(data.days) : null);
	const totalDays = $derived(data?.days.length ?? 0);
	const diff = $derived.by(() => {
		if (!avg || avg.intake === null || avg.budget === null) return '';
		const left = Math.round(avg.budget - avg.intake);
		return left >= 0 ? `${formatNumber(left)} left` : `${formatNumber(-left)} over`;
	});
	const caption = $derived.by(() => {
		if (!avg || avg.intake === null) return '';
		const budget = avg.budget === null ? '' : `Budget ${formatNumber(avg.budget)} kcal, ${diff} a day. `;
		return `${budget}${totalDays} days, ${avg.days} with meals.`;
	});
	const unit = $derived(session.profile?.weight_unit ?? 'kg');
	const typicalSteps = $derived(session.profile?.version?.typical_steps ?? null);
	const goalKg = $derived(session.profile?.version?.goal_kg ?? null);
</script>

<svelte:head><title>Trends · Cupful</title></svelte:head>

<div class="trends">
	<header class="page-head">
		<div>
			<h1>Trends</h1>
			<p class="muted small">{range} days</p>
		</div>
		<fieldset class="segmented ranges">
			<legend class="visually-hidden">Range</legend>
			{#each RANGES as r (r)}
				<label>
					<input type="radio" name="range" value={r} bind:group={range} />
					<span aria-hidden="true">{r}d</span><span class="visually-hidden">{r} days</span>
				</label>
			{/each}
		</fieldset>
	</header>

	{#if error}
		<div class="notice notice-error" role="alert">
			<p>{error}</p>
			<div class="row"><button class="btn" onclick={() => retry++}>Try again</button></div>
		</div>
	{:else if data === null}
		<div aria-busy="true" class="stack">
			<div class="skeleton" style="height: 7rem"></div>
			<div class="skeleton" style="height: 8rem"></div>
			<div class="skeleton" style="height: 16rem"></div>
		</div>
	{:else}
		{#key range}
			<section class="card summary" aria-label="Average intake against budget">
				<p class="small muted">Average a day</p>
				{#if avg && avg.intake !== null}
					<p class="lead">
						<CountUp value={avg.intake} />
						<span class="unit">kcal eaten</span>
					</p>
					<p class="small muted">{caption}</p>
				{:else}
					<p class="lead"><span class="num dash">—</span><span class="unit">kcal eaten</span></p>
					<p class="small muted">
						No meals logged in this range. Log a meal on Today and its day shows up here.
					</p>
				{/if}
			</section>

			<Consistency days={data.days} />

			<Charts
				days={data.days}
				today={today()}
				{unit}
				{typicalSteps}
				{goalKg}
			/>

			<Analytics
				averages={data.averages}
				mealMix={data.meal_mix}
				{targets}
				days={range}
			/>

			<p class="footnote">Past budgets use the weights recorded for those days.</p>
		{/key}
	{/if}
</div>

<style>
	.trends {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.page-head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
	}
	h1 {
		margin: 0;
	}
	.page-head p {
		margin: var(--space-1) 0 0;
	}
	.ranges {
		flex: 0 0 auto;
		min-width: 15rem;
	}
	.summary {
		background: var(--accent-soft);
		border-color: transparent;
	}
	.summary p {
		margin: 0;
	}
	.summary .lead {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		flex-wrap: wrap;
		margin: var(--space-1) 0 var(--space-2);
	}
	.lead :global(.num) {
		font-size: var(--text-hero);
		font-weight: var(--weight-bold);
		line-height: 1;
		letter-spacing: -0.03em;
		color: var(--accent);
	}
	.unit {
		font-size: var(--text-md);
		color: var(--text-muted);
	}
	.footnote {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	@media (max-width: 30rem) {
		.page-head {
			align-items: stretch;
		}
		.ranges {
			width: 100%;
		}
	}
</style>
