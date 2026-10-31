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
	import { errorMessage, getMeals, type Macros, type Meal } from '#lib/api.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { mealDelete, mealUpsert, sendOps, type Op } from '#lib/sync/index.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import { dataVersion } from '../offline/state.svelte.ts';
	import Review from './Review.svelte';
	import {
		MEAL_TYPES,
		capitalize,
		dayTotals,
		draftFromMeal,
		itemSourceLabel,
		mealSourceLabel,
		repeatMeal,
		type Draft
	} from './meal';

	let { date, refreshKey, onChanged }: { date: string; refreshKey: number; onChanged: () => void } = $props();

	let meals = $state<Meal[] | null>(null);
	let loadError = $state('');
	let status = $state('');
	let actionError = $state<{ id: string; message: string } | null>(null);
	let confirming = $state<string | null>(null);
	let editing = $state<Draft | null>(null);
	let busy = $state(false);

	let retry = $state(0);
	let shown = ''; // the date `meals` belongs to

	$effect(() => {
		const d = date;
		void refreshKey;
		void dataVersion.n;
		void retry;
		if (!d) return;
		if (d !== shown) meals = null; // a new day shows the skeleton; a refresh keeps the list
		shown = d;
		let stale = false;
		loadError = '';
		getMeals(d)
			.then((m) => !stale && (meals = m))
			.catch((e) => !stale && (loadError = errorMessage(e)));
		return () => (stale = true);
	});

	const groups = $derived(
		MEAL_TYPES.map((t) => ({ type: t, meals: (meals ?? []).filter((m) => m.meal_type === t) })).filter(
			(g) => g.meals.length
		)
	);
	const total = $derived(dayTotals(meals ?? []));

	async function send(meal: Meal, op: Op, done: string) {
		actionError = null;
		busy = true;
		try {
			const [r] = await sendOps([op]);
			if (r.status === 'rejected') throw new Error(`The change wasn't saved: ${r.error ?? 'rejected'}.`);
			if (r.status === 'stale') {
				actionError = { id: meal.id, message: staleNotice('meal') };
				confirming = null;
				onChanged();
				return;
			}
			status = r.status === 'queued' ? `${done} Waiting to sync.` : done;
			confirming = null;
			onChanged();
		} catch (e) {
			actionError = { id: meal.id, message: errorMessage(e) };
		} finally {
			busy = false;
		}
	}

	const title = (m: Meal) => m.description || m.items.map((i) => i.name).join(', ') || 'Meal';
	const macros = (t: Macros) =>
		`P ${formatGrams(t.protein_g)} · C ${formatGrams(t.carbs_g)} · F ${formatGrams(t.fat_g)} g`;
	const sourceChip = (m: Meal) => {
		const label = mealSourceLabel(m);
		return label === 'AI estimate' ? 'chip-ai' : label === 'Product' ? 'chip-db' : '';
	};
</script>

<section class="card meals" aria-labelledby="meals-title" aria-busy={meals === null && !loadError}>
	<header class="panel-head">
		<h2 id="meals-title">Meals</h2>
		{#if meals?.length}
			<p class="day-total" data-testid="day-total">
				<span class="num total-kcal">{formatKcal(total.kcal)} kcal</span>
				<span class="small muted num">{macros(total)}</span>
			</p>
		{/if}
	</header>
	<p class="visually-hidden" role="status">{status}</p>

	{#if loadError}
		<div class="notice notice-error" role="alert">
			<p>{loadError}</p>
			<div class="row"><button class="btn" onclick={() => retry++}>Try again</button></div>
		</div>
	{:else if meals === null}
		<div class="stack">
			{#each [0, 1] as i (i)}<div class="skeleton" style="height: 4.5rem"></div>{/each}
		</div>
	{:else if !meals.length}
		<p class="empty">Nothing logged yet. Describe what you ate above.</p>
	{:else}
		<ul class="meal-list">
			{#each groups.flatMap((g) => g.meals) as m (m.id)}
				<li
					class="meal-row"
					data-testid="meal"
					style="--m: var(--meal-{m.meal_type}); --m-soft: var(--meal-{m.meal_type}-soft); --m-text: var(--meal-{m.meal_type}-text)"
				>
					<details>
						<summary>
							<span class="dot" aria-hidden="true"></span>
							<span class="body">
								<span class="line">
									<span class="title">{title(m)}</span>
									<span class="num kcal">{formatKcal(m.totals.kcal)} kcal</span>
								</span>
								<span class="meta small">
									<span class="type">{capitalize(m.meal_type)}</span>
									<span class="muted num">{macros(m.totals)}</span>
									<span class="chip {sourceChip(m)}">{mealSourceLabel(m)}</span>
									{#if m.scale !== 1}<span class="chip chip-warning">×{m.scale}</span>{/if}
								</span>
							</span>
						</summary>
						<ul class="list items">
							{#each m.items as i (i.id)}
								{@const src = itemSourceLabel(i, m)}
								<li>
									<span class="item-what">
										<span>{i.name}</span>
										<span class="row small muted">
											{#if i.portion_text}<span>{i.portion_text}</span>{/if}
											{#if i.grams !== null}<span class="num">{formatGrams(i.grams)} g</span>{/if}
											{#if src}
												<span class="chip {src.kind === 'plain' ? '' : `chip-${src.kind}`}">{src.text}</span>
											{/if}
										</span>
									</span>
									<span class="sum">
										<span class="num">{formatKcal(i.kcal)} kcal</span>
										<span class="small muted num">{macros(i)}</span>
									</span>
								</li>
							{/each}
						</ul>
						{#if confirming === m.id}
							<div class="notice notice-warning confirm" role="group" aria-label="Confirm delete">
								<p>Delete this meal? This can't be undone.</p>
								<div class="row">
									<button class="btn btn-danger" disabled={busy} onclick={() => send(m, mealDelete(m.id), 'Meal deleted.')}
										>Delete meal</button
									>
									<button class="btn btn-quiet" onclick={() => (confirming = null)}>Keep it</button>
								</div>
							</div>
						{:else}
							<div class="row actions">
								<button
									class="btn"
									disabled={busy}
									onclick={() => send(m, mealUpsert(repeatMeal(m, date)), 'Logged again.')}>Eat this again</button
								>
								<button class="btn btn-quiet" onclick={() => (editing = draftFromMeal(m))}>Edit</button>
								<button class="btn btn-quiet danger" onclick={() => (confirming = m.id)}>Delete</button>
							</div>
						{/if}
						{#if actionError?.id === m.id}
							<p class="notice notice-error" role="alert">{actionError.message}</p>
						{/if}
					</details>
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if editing}
	<Review
		draft={editing}
		title="Edit meal"
		onClose={() => (editing = null)}
		onSaved={() => ((status = 'Meal updated.'), onChanged())}
		onStale={onChanged}
	/>
{/if}

<style>
	.meals {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.panel-head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-3);
		flex-wrap: wrap;
		margin-bottom: var(--space-1);
	}
	.day-total,
	.sum {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		text-align: right;
	}
	.total-kcal {
		font-weight: var(--weight-bold);
	}
	.meal-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.meal-row {
		border-bottom: 1px solid var(--border);
	}
	.meal-row:last-child {
		border-bottom: 0;
	}
	summary {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-1);
		min-height: var(--touch);
		cursor: pointer;
		list-style: none;
		border-radius: var(--radius-md);
		transition: background var(--dur-fast) var(--ease);
	}
	summary:hover {
		background: var(--surface-2);
	}
	summary:active {
		background: var(--m-soft);
	}
	summary::-webkit-details-marker {
		display: none;
	}
	.dot {
		width: 0.5625rem;
		height: 0.5625rem;
		margin-top: 0.45rem;
		border-radius: var(--radius-pill);
		background: var(--m);
		outline: 4px solid var(--m-soft);
		flex: none;
	}
	.body,
	.item-what {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}
	.line {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		min-width: 0;
	}
	.title {
		font-weight: var(--weight-medium);
		overflow-wrap: anywhere;
	}
	.kcal {
		flex: none;
		font-weight: var(--weight-bold);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
	}
	.type {
		color: var(--m-text);
		font-weight: var(--weight-medium);
	}
	.sum {
		flex: none;
	}
	details[open] summary {
		border-bottom: 1px solid var(--border);
		border-radius: var(--radius-md) var(--radius-md) 0 0;
	}
	.items,
	.actions,
	.confirm,
	details > .notice-error {
		margin: 0 0 var(--space-3);
	}
	.items > li {
		justify-content: space-between;
		align-items: flex-start;
	}
	.actions {
		padding-top: var(--space-1);
	}
	.item-what span:first-child {
		overflow-wrap: anywhere;
	}
	.danger {
		color: var(--error);
	}
	.danger:hover {
		background: var(--error-soft);
		color: var(--error);
	}
</style>
