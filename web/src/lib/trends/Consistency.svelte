<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Consistency — a calendar with one cell per day in the range: within budget, over, or no log. The
  classification is `consistency()` in logic.ts. The cells pop in one after another, and again when
  the range changes (the grid is keyed on the number of days).
-->
<script lang="ts">
	import type { TrendDay } from '../api';
	import { consistency, consistencyColumns, shortDate } from './logic.ts';

	let { days }: { days: TrendDay[] } = $props();

	const c = $derived(consistency(days));
	const columns = $derived(consistencyColumns(days.length));
	const word = (kind: string) =>
		kind === 'in'
			? 'within budget'
			: kind === 'over'
				? 'over budget'
				: kind === 'unknown'
					? 'logged, no budget'
					: 'no log';
	const summary = $derived(
		`${days.length} days: ${c.logged} logged, ${c.in} within budget, ${c.over} over, ${c.none} with no log.`
	);
</script>

<section class="card consistency" aria-labelledby="consistency-h">
	<div class="card-head">
		<h2 id="consistency-h">Consistency</h2>
		<span>
			<span class="num">{c.in}</span> within · <span class="num">{c.over}</span> over · <span class="num">{c.none}</span> no log{#if c.unknown}
				· <span class="num">{c.unknown}</span> no budget{/if}
		</span>
	</div>
	{#key days.length}
		<div class="grid" role="img" aria-label={summary} style:--cols={columns}>
			{#each c.cells as cell, i (cell.date)}
				<i class="{cell.kind} pop" style:--n={Math.min(i, 30) * 0.5} title="{shortDate(cell.date)}: {word(cell.kind)}" aria-hidden="true"></i>
			{/each}
		</div>
	{/key}
	<p class="legend">
		<span class="key"><i class="swatch in"></i>Within budget</span>
		<span class="key"><i class="swatch over"></i>Over budget</span>
		<span class="key"><i class="swatch none"></i>No log</span>
		{#if c.unknown}<span class="key"><i class="swatch unknown"></i>No budget</span>{/if}
	</p>
</section>

<style>
	.consistency {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.card-head span {
		text-align: right;
	}
	.card-head .num {
		font-weight: var(--weight-bold);
		color: var(--text);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
		gap: 5px;
	}
	.grid i {
		aspect-ratio: 1;
		max-width: 2.5rem;
		border-radius: 5px;
	}
	.in {
		background: var(--under);
	}
	.over {
		background: var(--over);
	}
	.unknown {
		background: var(--border-strong);
	}
	.none {
		background: transparent;
		box-shadow: inset 0 0 0 1.5px var(--border-strong);
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	.key {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.swatch {
		display: inline-block;
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 3px;
	}
</style>
