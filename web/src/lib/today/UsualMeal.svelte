<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UsualMeal — one calm chip for the current slot, from the last 60 days of meal history.
  Props:
    date: string          YYYY-MM-DD the shown day; the chip only renders for today
    refreshKey: number    bumped after a write; refetch the history
    onSaved: () => void   a meal was logged or undone
  Log saves a fresh repeat with mealUpsert + sendOps, no Review. The chip body opens Review with
  the usual filled in. The menu lists close alternatives and a local "Don't suggest this".
-->
<script lang="ts">
	import { errorMessage, getMealHistory, type HistoryMeal } from '#lib/api.ts';
	import { dismiss } from '#lib/dismiss.ts';
	import { formatKcal } from '#lib/format.ts';
	import { queueState } from '#lib/offline/state.svelte.ts';
	import { defaultMealType } from '#lib/products/logic.ts';
	import { today } from '#lib/session.svelte.ts';
	import { sendOps } from '#lib/sync/index.ts';
	import { staleNotice } from '#lib/sync/stale.ts';
	import Review from './Review.svelte';
	import { repeatDraft, repeatTitle, type Draft } from './meal';
	import {
		USUAL_DAYS,
		USUAL_LIMIT,
		dismissUsual,
		pickUsual,
		readDismissed,
		usualAlternatives,
		usualLogOp,
		usualUndoOp
	} from './usual.ts';

	let {
		date,
		refreshKey,
		onSaved
	}: { date: string; refreshKey: number; onSaved: () => void } = $props();

	let stamp = $state(Date.now());
	let meals = $state<HistoryMeal[] | null>(null);
	let dismissed = $state<string[]>(readDismissed());
	let saving = $state(false);
	let undoBusy = $state(false);
	let error = $state('');
	let hidden = $state(false);
	let logged = $state<{ id: string; queued: boolean } | null>(null);
	let review = $state<{ draft: Draft; title: string } | null>(null);

	// The slot is recomputed from a clock that wakes with the app, so a tab left open from
	// breakfast stops offering breakfast at lunch.
	const slot = $derived(defaultMealType(new Date(stamp).getHours()));
	const usual = $derived(
		!hidden && date === today() && meals
			? pickUsual(meals, slot, today(), dismissed, queueState.ops)
			: null
	);
	const alternatives = $derived(
		usual && meals ? usualAlternatives(meals, slot, usual, dismissed) : []
	);

	$effect(() => {
		const wake = () => (stamp = Date.now());
		const onVisible = () => {
			if (!document.hidden) wake();
		};
		document.addEventListener('visibilitychange', onVisible);
		window.addEventListener('focus', wake);
		return () => {
			document.removeEventListener('visibilitychange', onVisible);
			window.removeEventListener('focus', wake);
		};
	});

	$effect(() => {
		const shown = date;
		void refreshKey;
		if (shown !== today()) return;
		let stale = false;
		getMealHistory(USUAL_LIMIT, { days: USUAL_DAYS, meal_type: slot, sort: 'frequent' })
			.then((m) => !stale && (meals = m))
			.catch(() => {
				// A missing chip is calm; the composer still works.
				if (!stale) meals = [];
			});
		return () => (stale = true);
	});

	function openReview(meal: HistoryMeal) {
		error = '';
		review = { draft: repeatDraft(meal, date), title: repeatTitle(date, today()) };
	}

	async function log() {
		if (!usual || saving) return;
		error = '';
		saving = true;
		const op = usualLogOp(usual, date);
		const id = op.kind === 'meal.upsert' ? op.payload.id : '';
		try {
			const [r] = await sendOps([op]);
			if (r.status === 'rejected') {
				error = `The usual wasn't saved: ${r.error ?? 'rejected'}.`;
			} else if (r.status === 'stale') {
				error = staleNotice('meal');
			} else {
				logged = { id, queued: r.status === 'queued' };
				onSaved();
			}
		} catch (e) {
			error = errorMessage(e);
		} finally {
			saving = false;
		}
	}

	async function undo() {
		if (!logged || undoBusy) return;
		error = '';
		undoBusy = true;
		try {
			const [r] = await sendOps([usualUndoOp(logged.id)]);
			if (r.status === 'rejected') {
				error = `The undo wasn't saved: ${r.error ?? 'rejected'}.`;
			} else if (r.status === 'stale') {
				error = staleNotice('meal');
			} else {
				logged = null;
				onSaved();
			}
		} catch (e) {
			error = errorMessage(e);
		} finally {
			undoBusy = false;
		}
	}

	function dismissTop() {
		if (!usual) return;
		dismissed = dismissUsual(slot, usual.label, dismissed);
	}

	function reviewSaved() {
		review = null;
		hidden = true;
		onSaved();
	}
</script>

{#if date === today() && (logged || usual)}
	<section
		class="usual"
		data-testid="usual"
		aria-label="Usual meal"
		style="--m: var(--meal-{slot}); --m-soft: var(--meal-{slot}-soft); --m-text: var(--meal-{slot}-text)"
	>
		{#if logged}
			<div class="notice notice-info row" role="status">
				<p>{logged.queued ? 'Usual queued. Waiting to sync.' : 'Usual logged.'}</p>
				<button class="btn btn-quiet" data-testid="usual-undo" disabled={undoBusy} onclick={undo}
					>Undo</button
				>
			</div>
		{:else if usual}
			<div class="usual-chip">
				<button class="usual-body" onclick={() => openReview(usual)}>
					<span class="usual-label">{usual.label}</span>
					<span class="small muted num">{formatKcal(usual.totals.kcal)} kcal</span>
				</button>
				<button class="btn btn-primary" disabled={saving} onclick={log}>
					{saving ? 'Saving…' : 'Log'}
				</button>
				<details class="menu" use:dismiss>
					<summary class="btn btn-quiet" aria-label="More usual options">⋯</summary>
					<div class="menu-panel">
						<p class="small muted">Other usuals</p>
						{#each alternatives as alt (alt.id)}
							<button class="btn btn-quiet btn-block" data-close onclick={() => openReview(alt)}>
								{alt.label} · {formatKcal(alt.totals.kcal)} kcal
							</button>
						{:else}
							<p class="small muted">No other usuals yet.</p>
						{/each}
						<button class="btn btn-quiet btn-block danger" data-close onclick={dismissTop}>
							Don't suggest this
						</button>
					</div>
				</details>
			</div>
		{/if}
		{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}
	</section>
{/if}

{#if review}
	{#key review.draft.id}
		<Review
			draft={review.draft}
			title={review.title}
			onClose={() => (review = null)}
			onSaved={reviewSaved}
		/>
	{/key}
{/if}

<style>
	.usual-chip {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
		background: var(--m-soft);
		border: 1px solid color-mix(in srgb, var(--m) 30%, transparent);
		border-radius: var(--radius-lg);
	}
	.usual-body {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: flex-start;
		min-width: 0;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--m-text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.usual-label {
		overflow: hidden;
		max-width: 100%;
		font-weight: var(--weight-medium);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.usual-body .small {
		color: var(--text-muted);
	}
	.menu > summary {
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
	.menu-panel {
		left: auto;
		right: 0;
		min-width: 14rem;
	}
</style>
