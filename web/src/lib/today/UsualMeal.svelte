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
			<div class="undo pop" role="status">
				<span class="ok" aria-hidden="true">
					<svg viewBox="0 0 24 24"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg>
				</span>
				<p>{logged.queued ? 'Usual queued. Waiting to sync.' : 'Usual logged.'}</p>
				<button class="btn btn-quiet" data-testid="usual-undo" disabled={undoBusy} onclick={undo}
					>Undo</button
				>
			</div>
		{:else if usual}
			<div class="usual-chip rise">
				<button class="usual-body" onclick={() => openReview(usual)}>
					<span class="usual-ico" aria-hidden="true">
						<svg viewBox="0 0 24 24"
							><path d="M7 3v8M5 3v5a2 2 0 0 0 4 0V3M7 11v10M16 3c-2 2-3 5-3 8h3v10" /></svg
						>
					</span>
					<span class="usual-txt">
						<span class="usual-label">{usual.label}</span>
						<span class="small muted num"
							>Your usual {slot} · {formatKcal(usual.totals.kcal)} kcal</span
						>
					</span>
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
	.usual-chip,
	.undo {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: 4.5rem;
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
	}
	.usual-chip {
		background: var(--surface);
	}
	.usual-body {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-3);
		min-width: 0;
		min-height: var(--touch);
		padding: 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
		transition: scale var(--dur-fast) var(--ease);
	}
	.usual-body:active {
		scale: 0.97;
	}
	.usual-body:focus-visible {
		outline: 3px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.usual-ico {
		display: grid;
		place-items: center;
		flex: none;
		inline-size: 2.5rem;
		block-size: 2.5rem;
		border-radius: var(--radius-md);
		background: var(--m-soft);
		color: var(--m-text);
	}
	.usual-ico svg,
	.ok svg {
		inline-size: 1.25rem;
		block-size: 1.25rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.usual-txt {
		display: grid;
		min-width: 0;
	}
	.usual-txt .small {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.usual-label {
		overflow: hidden;
		font-weight: var(--weight-bold);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.undo {
		background: var(--under-soft);
		border-color: transparent;
	}
	.undo p {
		flex: 1;
		margin: 0;
		padding-inline: var(--space-2);
		font-weight: var(--weight-bold);
	}
	.ok {
		display: grid;
		place-items: center;
		flex: none;
		inline-size: 1.75rem;
		block-size: 1.75rem;
		border-radius: 50%;
		background: var(--under);
		color: var(--surface);
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
