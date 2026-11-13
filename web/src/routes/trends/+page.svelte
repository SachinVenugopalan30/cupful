<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { errorMessage, getBudget, getTrends, type Budget, type Trends } from '#lib/api.ts';
	import { session, today } from '#lib/session.svelte.ts';
	import { dataVersion } from '#lib/offline/state.svelte.ts';
	import { loadRange, RANGES, rangeDates, saveRange, type Range } from '#lib/trends/logic.ts';
	import Consistency from '#lib/trends/Consistency.svelte';
	import FrequentMeals from '#lib/trends/FrequentMeals.svelte';
	import IntakeChart from '#lib/trends/IntakeChart.svelte';
	import MacroBalance from '#lib/trends/MacroBalance.svelte';
	import MealMix from '#lib/trends/MealMix.svelte';
	import StepsChart from '#lib/trends/StepsChart.svelte';
	import WeightChart from '#lib/trends/WeightChart.svelte';

	let range = $state<Range>(loadRange());
	// The range the screen shows. It lags `range` while a new one loads, so the old charts stay up
	// (no skeleton flash) and the average can count from its old value to the new one.
	let shown = $state<{ range: Range; data: Trends } | null>(null);
	let targets = $state<Budget | null>(null);
	let error = $state('');
	let retry = $state(0);

	$effect(() => {
		const r = range;
		saveRange(r);
		void dataVersion.n;
		void retry;
		const { from, to } = rangeDates(r, today());
		let stale = false;
		error = '';
		getTrends(from, to).then(
			(d) => !stale && (shown = { range: r, data: d }),
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

	const unit = $derived(session.profile?.weight_unit ?? 'kg');
	const typicalSteps = $derived(session.profile?.version?.typical_steps ?? null);
	const goalKg = $derived(session.profile?.version?.goal_kg ?? null);
</script>

<svelte:head><title>Trends · Cupful</title></svelte:head>

<div class="trends">
	<header class="page-head">
		<h1>Trends</h1>
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
	{:else if shown === null}
		<div aria-busy="true" class="stack">
			<div class="skeleton" style="height: 14rem"></div>
			<div class="skeleton" style="height: 10rem"></div>
			<div class="skeleton" style="height: 16rem"></div>
		</div>
	{:else}
		{@const d = shown.data}
		<div class="grid" aria-busy={shown.range !== range}>
			<div class="intake"><IntakeChart days={d.days} today={today()} /></div>
			<div class="consistency"><Consistency days={d.days} /></div>
			<div class="weight"><WeightChart days={d.days} today={today()} {unit} {goalKg} /></div>
			<div class="steps"><StepsChart days={d.days} today={today()} {typicalSteps} /></div>
			<div class="mix"><MealMix mealMix={d.meal_mix} days={shown.range} /></div>
			<div class="macros"><MacroBalance averages={d.averages} {targets} days={shown.range} /></div>
			{#if d.top_meals}<div class="frequent"><FrequentMeals meals={d.top_meals} days={shown.range} /></div>{/if}
		</div>
		<p class="footnote">Past budgets use the weights recorded for those days.</p>
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
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
	}
	h1 {
		margin: 0;
	}
	.ranges {
		flex: 0 0 auto;
		min-width: 15rem;
	}
	/* Phone: one column in the order intake, consistency, weight, steps, meal mix, macros, eaten most. */
	.grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-3);
		align-items: start;
	}
	.grid > * {
		min-width: 0;
	}
	.footnote {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	/* Desktop: wide charts span two of three columns, each with a narrow card beside it. */
	@media (min-width: 60rem) {
		.grid {
			grid-template-columns: repeat(3, minmax(0, 1fr));
			grid-template-areas:
				'intake intake consistency'
				'weight weight mix'
				'steps steps macros'
				'frequent frequent frequent';
			gap: var(--space-5);
		}
		.intake {
			grid-area: intake;
		}
		.consistency {
			grid-area: consistency;
		}
		.weight {
			grid-area: weight;
		}
		.mix {
			grid-area: mix;
		}
		.steps {
			grid-area: steps;
		}
		.macros {
			grid-area: macros;
		}
		.frequent {
			grid-area: frequent;
		}
	}
	@media (max-width: 30rem) {
		.ranges {
			width: 100%;
		}
	}
</style>
