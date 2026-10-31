<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Consistency — one cell per day in the range: within budget, over, or no log. The grid keeps
  30 columns for the long ranges so 365 days stay a compact block.
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
	<div class="head">
		<h2 id="consistency-h">Consistency</h2>
		<span class="muted small">{c.logged} of {days.length} days logged</span>
	</div>
	<p class="counts small">
		<strong class="num">{c.in}</strong> within budget ·
		<strong class="num">{c.over}</strong> over ·
		<strong class="num">{c.none}</strong> no log{#if c.unknown}
			· <strong class="num">{c.unknown}</strong> no budget
		{/if}
	</p>
	<div class="grid" role="img" aria-label={summary} style:--cols={columns}>
		{#each c.cells as cell, i (cell.date)}
			<i
				class={cell.kind}
				style:--i={Math.min(i, 40)}
				title="{shortDate(cell.date)}: {word(cell.kind)}"
				aria-hidden="true"
			></i>
		{/each}
	</div>
	<p class="legend small muted">
		<span><i class="swatch in"></i>Within budget</span>
		<span><i class="swatch over"></i>Over budget</span>
		<span><i class="swatch none"></i>No log</span>
		{#if c.unknown}<span><i class="swatch unknown"></i>No budget</span>{/if}
	</p>
</section>

<style>
	.consistency {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
	}
	h2 {
		margin: 0;
	}
	.counts {
		margin: 0;
	}
	.counts strong {
		font-weight: var(--weight-bold);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
		gap: var(--space-1);
		margin: var(--space-1) 0;
	}
	.grid i {
		width: 100%;
		max-width: 2.25rem;
		aspect-ratio: 1;
		border-radius: 0.1875rem;
		background: var(--border);
		animation: pop 280ms var(--ease) both;
		animation-delay: calc(var(--i) * 14ms);
	}
	.grid i.in {
		background: var(--accent);
	}
	.grid i.over {
		background: var(--chart-over);
	}
	.grid i.unknown {
		background: var(--border-strong);
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
	}
	.legend span {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.swatch {
		display: inline-block;
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 0.125rem;
	}
	.swatch.in {
		background: var(--accent);
	}
	.swatch.over {
		background: var(--chart-over);
	}
	.swatch.none {
		background: var(--border);
	}
	.swatch.unknown {
		background: var(--border-strong);
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: scale(0.55);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.grid i {
			animation: none;
		}
	}
</style>
