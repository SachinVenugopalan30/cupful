<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  BudgetCard — eaten vs budget, left/over, macro bars, flags, "how it's worked out".
  Props (fixed contract; Today page passes these):
    date: string        YYYY-MM-DD, the selected day
    refreshKey: number  bumped after any write to the day; refetch when it changes
  The server computes the budget; this card only sums the day's meals for "eaten".
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { ApiError, errorMessage, getBudget, getMeals, type Budget, type Macros } from '#lib/api.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { session } from '#lib/session.svelte.ts';
	import { breakdown, flagMessages, leftOver, macroRows } from './budget.ts';
	import { dayTotals } from './meal.ts';

	let { date, refreshKey }: { date: string; refreshKey: number } = $props();

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
		void attempt;
		// A different day: show the skeleton, not the last day's numbers. Same day: keep them.
		untrack(() => {
			if (s.kind !== 'ready' || s.budget.date !== date) s = { kind: 'loading' };
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
</script>

<section class="card stack" aria-labelledby="budget-h" aria-busy={s.kind === 'loading'}>
	<h2 id="budget-h">Budget</h2>
	{#if s.kind === 'loading'}
		<div class="skeleton hero-skel"></div>
		<div class="skeleton"></div>
		<div class="skeleton"></div>
		<div class="skeleton"></div>
	{:else if s.kind === 'no-profile'}
		<div class="empty stack">
			<p>Set up your profile to see your daily budget. It needs your height, weight and goal.</p>
			<p><a class="btn btn-primary" href="/settings#profile-h">Set up your profile</a></p>
		</div>
	{:else if s.kind === 'error'}
		<div class="notice notice-error" role="alert">
			<p>{s.message}</p>
			<p><button class="btn" onclick={() => attempt++}>Try again</button></p>
		</div>
	{:else}
		{@const b = s.budget}
		{@const lo = leftOver(s.eaten.kcal, b.budget)}
		<div class="hero" class:is-over={lo.over}>
			<p class="left num">{formatKcal(lo.kcal)}</p>
			<p class="left-label">kcal {lo.over ? 'over' : 'left'}</p>
		</div>
		<div
			class="meter"
			class:is-over={lo.over}
			style="--value: {b.budget ? s.eaten.kcal / b.budget : 0}"
			role="meter"
			aria-label="Calories eaten"
			aria-valuemin="0"
			aria-valuemax={b.budget}
			aria-valuenow={Math.round(s.eaten.kcal)}
			aria-valuetext="{formatKcal(s.eaten.kcal)} of {formatKcal(b.budget)} kcal"
		></div>
		<p class="small muted">
			<span class="num">{formatKcal(s.eaten.kcal)}</span> eaten of
			<span class="num">{formatKcal(b.budget)}</span> kcal budget
		</p>

		{#if b.flags.length}
			<ul class="flags stack">
				{#each flagMessages(b.flags) as msg (msg)}
					<li class="notice notice-warning">{msg}</li>
				{/each}
			</ul>
		{/if}

		<ul class="macros stack">
			{#each macroRows(b, s.eaten) as m (m.key)}
				<li class="stack">
					<div class="macro-head">
						<span class="macro-name" style="--c: var(--{m.key.slice(0, -2)}-text)">{m.label}</span>
						<span class="num small">
							{#if m.target === null}
								{formatGrams(m.eaten)} g · no target today
							{:else}
								{formatGrams(m.eaten)} / {formatGrams(m.target)} g{m.over ? ' · over' : ''}
							{/if}
						</span>
					</div>
					{#if m.target !== null}
						<div
							class="meter"
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

		<details>
			<summary>How it's worked out</summary>
			<dl class="terms">
				{#each breakdown(b, unit) as t (t.label)}
					<div>
						<dt>{t.label}</dt>
						<dd class="num">{t.value}</dd>
						<dd class="note small muted">{t.note}</dd>
					</div>
				{/each}
			</dl>
			<p class="small muted">
				Budgets are worked out again from current data each time. Past days use the profile and
				weights recorded for those days, so changing your profile today doesn't change them.
			</p>
		</details>
	{/if}
</section>

<style>
	.hero {
		color: var(--under);
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
	}
	.hero.is-over {
		color: var(--over);
	}
	.left {
		font-size: var(--text-hero);
		font-weight: var(--weight-bold);
		line-height: 1;
	}
	.left-label {
		font-size: var(--text-lg);
		font-weight: var(--weight-medium);
	}
	.hero-skel {
		height: var(--text-hero);
		width: 60%;
	}
	.flags,
	.macros {
		list-style: none;
		margin: 0;
		padding: 0;
		gap: var(--space-3);
	}
	.macros > li {
		gap: var(--space-1);
	}
	.macro-name {
		color: var(--c);
		font-weight: var(--weight-medium);
	}
	.macro-head {
		display: flex;
		justify-content: space-between;
		gap: var(--space-2);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
	summary {
		min-height: var(--touch);
		display: flex;
		align-items: center;
		color: var(--accent);
		font-weight: var(--weight-medium);
		cursor: pointer;
	}
	.terms {
		margin: 0 0 var(--space-3);
	}
	.terms > div {
		display: grid;
		grid-template-columns: 1fr auto;
		column-gap: var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}
	dt {
		font-weight: var(--weight-medium);
	}
	dd {
		margin: 0;
		text-align: right;
	}
	.terms .note {
		grid-column: 1 / -1;
		text-align: left;
	}
</style>
