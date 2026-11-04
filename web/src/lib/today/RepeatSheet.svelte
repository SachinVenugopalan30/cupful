<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  RepeatSheet — recent distinct meals for the selected day. Picking one closes the
  sheet and hands the meal and search query to the composer, which opens Review.
  Props: date, query, onClose, onPick.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { errorMessage, getMealHistory, type HistoryMeal } from '#lib/api.ts';
	import { dayLabel } from '#lib/date.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { closeSheet } from '#lib/sheet.ts';
	import { today } from '#lib/session.svelte.ts';
	import { filterHistory } from './meal';

	let {
		date,
		query = '',
		onClose,
		onPick
	}: {
		date: string;
		query?: string;
		onClose: () => void;
		onPick: (meal: HistoryMeal, query: string) => void;
	} = $props();

	let dialog: HTMLDialogElement;
	let returnFocus: HTMLElement | null = null;
	let heading: HTMLHeadingElement;
	let meals = $state<HistoryMeal[] | null>(null);
	let error = $state('');
	// The prop seeds the search box; the user edits the local copy from here on.
	// svelte-ignore state_referenced_locally
	let q = $state(query);

	const shown = $derived(meals ? filterHistory(meals, q) : []);
	const target = $derived(dayLabel(date, today()));

	onMount(() => {
		returnFocus = document.activeElement as HTMLElement | null;
		dialog.showModal();
		heading?.focus();
		void load();
	});

	async function load() {
		error = '';
		try {
			meals = await getMealHistory(50);
		} catch (e) {
			error = errorMessage(e);
		}
	}

	function close() {
		closeSheet(dialog, () => {
			returnFocus?.focus();
			onClose();
		});
	}

	function pick(meal: HistoryMeal) {
		closeSheet(dialog, () => onPick(meal, q));
	}

	const macros = (m: HistoryMeal) =>
		`P ${formatGrams(m.totals.protein_g)} · C ${formatGrams(m.totals.carbs_g)} · F ${formatGrams(m.totals.fat_g)} g`;
	const eaten = (n: number) => (n === 1 ? 'once' : `${n} times`);
</script>

<dialog
	class="sheet"
	bind:this={dialog}
	oncancel={(event) => {
		event.preventDefault();
		close();
	}}
	aria-labelledby="repeat-title"
>
	<div class="stack">
		<header class="sheet-head">
			<h2 id="repeat-title" tabindex="-1" bind:this={heading}>Repeat onto {target}</h2>
			<button type="button" class="btn btn-quiet" onclick={close}>Cancel</button>
		</header>
		<p class="small muted">Recent meals, last year. Pick one, then adjust it before saving.</p>

		<div class="field">
			<label for="repeat-search">Search recent meals</label>
			<input id="repeat-search" type="search" bind:value={q} placeholder="Name or ingredient" />
		</div>

		{#if error}
			<div class="notice notice-error" role="alert">
				<p>{error}</p>
				<div class="row">
					<button type="button" class="btn" onclick={load}>Try again</button>
				</div>
			</div>
		{:else if meals === null}
			<div class="stack" aria-busy="true">
				{#each [0, 1, 2] as i (i)}<div class="skeleton" style="height: 3.5rem"></div>{/each}
			</div>
		{:else if !meals.length}
			<p class="empty">No meals in the last year. Log one and it will show up here.</p>
		{:else if !shown.length}
			<p class="empty">No recent meals match “{q.trim()}”.</p>
			<button type="button" class="btn" onclick={() => (q = '')}>Clear search</button>
		{:else}
			<ul class="list">
				{#each shown as m (m.id)}
					<li>
						<button
							type="button"
							class="pick"
							aria-label="{m.label}, {formatKcal(m.totals.kcal)} kcal, last {dayLabel(m.last_date, today())}, eaten {eaten(m.times)}"
							onclick={() => pick(m)}
						>
							<span class="name">{m.label}</span>
							<span class="small muted">
								{m.meal_type} · last {dayLabel(m.last_date, today())}
								{#if m.times > 1}· eaten {m.times} times{/if}
							</span>
							<span class="small muted num">{macros(m)}</span>
						</button>
						<span class="num kcal">{formatKcal(m.totals.kcal)} kcal</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</dialog>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0.125rem;
		min-width: 0;
		min-height: var(--touch);
		padding: var(--space-1) var(--space-2);
		margin-left: calc(-1 * var(--space-2));
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-state),
			border-color var(--dur-fast) var(--ease-state);
	}
	.pick:hover {
		background: var(--surface-2);
		border-color: var(--border);
	}
	.name {
		font-weight: var(--weight-medium);
		overflow-wrap: anywhere;
	}
	li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		border-bottom: 1px solid var(--border);
	}
	li:last-child {
		border-bottom: 0;
	}
	.kcal {
		flex: none;
		font-weight: var(--weight-medium);
	}
</style>
