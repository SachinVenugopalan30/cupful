<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  BudgetCard — eaten vs budget, left/over, macro bars, flags, "how it's worked out".
  Props (fixed contract; Today page passes these):
    date: string        YYYY-MM-DD, the selected day
    refreshKey: number  bumped after any write to the day; refetch when it changes
    budget, eaten       bindable: the loaded budget and the day's eaten kcal, for the cards beside it
  The server computes the budget; this card only sums the day's meals for "eaten". Before a profile
  exists the budget answers 409: the jug stays empty and the card points at /profile.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { ApiError, errorMessage, getBudget, getMeals, type Budget, type Macros } from '#lib/api.ts';
	import { dismiss } from '#lib/dismiss.ts';
	import CountUp from '#lib/motion/CountUp.svelte';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { session } from '#lib/session.svelte.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import { breakdown, flagMessages, leftOver, macroRows } from './budget.ts';
	import { dayTotals } from './meal.ts';
	import Jug from './Jug.svelte';
	import { jugFraction, jugLabel, jugMarks } from './jug.ts';

	// `budget` and `eaten` are optional and bindable: the Activity page reads the budget from here,
	// and Today's week card follows the eaten figure.
	let {
		date,
		refreshKey,
		budget = $bindable(null),
		eaten = $bindable(0)
	}: { date: string; refreshKey: number; budget?: Budget | null; eaten?: number } = $props();

	type State =
		| { kind: 'loading' }
		| { kind: 'no-profile' }
		| { kind: 'error'; message: string }
		| { kind: 'ready'; budget: Budget; eaten: Macros };
	let s = $state<State>({ kind: 'loading' });
	let attempt = $state(0);

	$effect(() => {
		if (!date) return;
		void refreshKey;
		void dataVersion.n;
		void attempt;
		// Keep the last numbers on screen while a different day loads. The card is read-only, so a
		// brief stale value is calmer than a skeleton flash.
		untrack(() => {
			if (s.kind !== 'ready') s = { kind: 'loading' };
		});
		let stale = false;
		Promise.all([getBudget(date), getMeals(date)]).then(
			([budget, meals]) => !stale && (s = { kind: 'ready', budget, eaten: dayTotals(meals) }),
			(e) =>
				!stale &&
				(s =
					e instanceof ApiError && e.status === 409
						? { kind: 'no-profile' }
						: { kind: 'error', message: errorMessage(e) })
		);
		return () => (stale = true);
	});

	const unit = $derived(session.profile?.weight_unit ?? 'kg');
	$effect(() => {
		budget = s.kind === 'ready' && s.budget.date === date ? s.budget : null;
		// Same date guard as `budget`: while another day loads, the old day's total must not leak out.
		eaten = s.kind === 'ready' && s.budget.date === date ? s.eaten.kcal : 0;
	});
</script>

<section class="card budget" aria-labelledby="budget-h" aria-busy={s.kind === 'loading'}>
	<h2 id="budget-h" class="visually-hidden">Budget</h2>
	{#if s.kind === 'ready'}
		{@const b = s.budget}
		{@const lo = leftOver(s.eaten.kcal, b.budget)}
		<div class="jug-row">
			<Jug
				fraction={jugFraction(s.eaten.kcal, b.budget)}
				marks={jugMarks(b.budget)}
				label={jugLabel(s.eaten.kcal, b.budget, b.manual)}
			/>
			<div class="side">
				<div class="lead" class:is-over={lo.over}>
					<p class="fig left"><CountUp value={lo.kcal} /></p>
					<p class="left-label">kcal {lo.over ? 'over' : 'left'}</p>
				</div>
				<p class="caption small muted">
					<CountUp value={Math.round(s.eaten.kcal)} /> eaten of
					<span class="num">{b.budget.toLocaleString('en-US')}</span> kcal {b.manual ? 'goal' : 'plan'}
				</p>
				<ul class="macros">
					{#each macroRows(b, s.eaten) as m (m.key)}
						<li style="--c: var(--{m.key.slice(0, -2)}-text)">
							<span class="macro-name">{m.label}</span>
							<span class="num small macro-val">
								{#if m.target === null}
									{formatGrams(m.eaten)} g · no target today
								{:else}
									{formatGrams(m.eaten)} / {formatGrams(m.target)} g{m.over ? ' · over' : ''}
								{/if}
							</span>
							{#if m.target !== null}
								<div
									class="meter macro-meter"
									class:is-over={m.over}
									style="--value: {m.value}; --fill: var(--{m.key.slice(0, -2)})"
									role="meter"
									aria-label={m.label}
									aria-valuemin="0"
									aria-valuemax={m.target}
									aria-valuenow={Math.round(m.eaten * 10) / 10}
									aria-valuetext="{formatGrams(m.eaten)} of {formatGrams(m.target)} g{m.over ? ', over' : ''}"
								></div>
							{/if}
						</li>
					{/each}
				</ul>
			</div>
		</div>

		{#if b.flags.length}
			<ul class="flags">
				{#each flagMessages(b) as msg (msg)}
					<li class="notice notice-warning">{msg}</li>
				{/each}
			</ul>
		{/if}

		<details class="menu working" use:dismiss={true}>
			<summary>How it's worked out</summary>
			<div class="menu-panel up" role="group" aria-label="How the budget is worked out">
				<dl class="terms">
					{#each breakdown(b, unit) as t (t.label)}
						<dt title={t.note}>{t.label}</dt>
						<dd class="num">{t.value}</dd>
					{/each}
				</dl>
				<p class="small muted">
					Worked out again from current data each time. Past days use the profile and weights
					recorded for them.
				</p>
				<button type="button" class="btn done" data-close>Close</button>
			</div>
		</details>
	{:else if s.kind === 'loading'}
		<div class="jug-row" aria-hidden="true">
			<Jug fraction={0} label="" />
			<div class="side">
				<div class="skeleton hero-skel"></div>
				<div class="skeleton"></div>
				<div class="skeleton"></div>
			</div>
		</div>
	{:else if s.kind === 'no-profile'}
		<div class="jug-row">
			<Jug fraction={0} label="Empty jug: no daily plan yet" />
			<div class="side stack">
				<p class="empty-title">Save your profile to see your daily plan.</p>
				<p class="small muted">It needs your height, weight and goal. You can log meals meanwhile.</p>
				<p><a class="btn btn-primary" href="/profile">Set up your profile</a></p>
			</div>
		</div>
	{:else}
		<div class="notice notice-error" role="alert">
			<p>{s.message}</p>
			<p><button class="btn" onclick={() => attempt++}>Try again</button></p>
		</div>
	{/if}
</section>

<style>
	.budget {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-4) var(--space-4) var(--space-3) var(--space-4);
		--jug-h: 14.75rem;
	}
	.jug-row {
		display: grid;
		grid-template-columns: 6.5rem minmax(0, 1fr);
		gap: var(--space-4);
		align-items: end;
	}
	.side {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}
	.lead {
		display: grid;
		gap: var(--space-1);
		color: var(--text);
	}
	.lead.is-over {
		color: var(--over);
	}
	/* The largest figure on the screen. */
	.left {
		font-size: 3.375rem;
	}
	.left-label {
		margin: 0;
		font-size: var(--text-md);
		font-weight: var(--weight-medium);
		color: var(--text-muted);
	}
	.lead.is-over .left-label {
		color: inherit;
	}
	.caption {
		margin: 0;
	}
	.hero-skel {
		height: 3.375rem;
		width: 70%;
	}
	.empty-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: 1.25rem;
		font-weight: 700;
		line-height: var(--leading-tight);
	}
	.flags,
	.macros {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.flags {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.macros {
		display: grid;
		gap: var(--space-2);
		margin-top: var(--space-1);
	}
	.macros > li {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		column-gap: var(--space-2);
		row-gap: var(--space-1);
		align-items: baseline;
	}
	.macro-name {
		color: var(--c);
		font-size: var(--text-xs);
		font-weight: var(--weight-bold);
	}
	.macro-val {
		text-align: right;
		color: var(--text-muted);
	}
	.macro-meter {
		grid-column: 1 / -1;
		height: 0.375rem;
	}
	.working {
		border-top: 1px solid var(--border);
		padding-top: var(--space-1);
		/* In a stretched card the row sits at the bottom. */
		margin-top: auto;
	}
	summary {
		min-height: var(--touch);
		display: flex;
		align-items: center;
		color: var(--accent-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		cursor: pointer;
		text-decoration: underline;
		text-underline-offset: 0.15em;
	}
	.terms {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 0 var(--space-4);
		margin: 0 0 var(--space-2);
		font-size: var(--text-xs);
		line-height: 1.7;
		font-variant-numeric: tabular-nums;
	}
	dt {
		color: var(--text-muted);
	}
	dd {
		margin: 0;
		text-align: right;
		font-weight: var(--weight-medium);
	}
	.working .done {
		width: 100%;
		margin-top: var(--space-2);
	}
	/* The 400px desktop column: a taller jug and more room for the figure. */
	@media (min-width: 60rem) {
		.budget {
			--jug-h: 17.5rem;
			padding: var(--space-5);
		}
		.jug-row {
			grid-template-columns: 7.5rem minmax(0, 1fr);
			gap: var(--space-5);
		}
	}
</style>
