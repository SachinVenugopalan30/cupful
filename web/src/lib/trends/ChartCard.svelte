<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ChartCard — the frame every Trends chart sits in: a title with a muted note on the right, the
  headline figure, a line of context, the chart with its axis, a legend, the value of the day last
  tapped (an aria-live line), and the empty state. The chart itself is presentational; the plot
  carries `role="img"` and the summary text.
  Props: title, aside? (right of the title), label? (region name when it differs from the title),
  summary (for the plot), readout?, empty?, figure? (the big number; keep CountUp outside any
  keyed block so it counts from the old value), note?, legend?, big? (a larger figure), children.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		aside = '',
		label = '',
		summary,
		readout = '',
		empty = '',
		big = false,
		figure,
		legend,
		note,
		children
	}: {
		title: string;
		aside?: string;
		label?: string;
		summary: string;
		readout?: string;
		empty?: string;
		big?: boolean;
		figure?: Snippet;
		legend?: Snippet;
		note?: Snippet;
		children: Snippet;
	} = $props();
	const id = $props.id();
</script>

<section class="card chart-card" aria-label={label || undefined} aria-labelledby={label ? undefined : `${id}-h`}>
	<div class="card-head">
		<h2 id="{id}-h">{title}</h2>
		{#if aside}<span>{aside}</span>{/if}
	</div>
	{#if figure && !empty}<p class="fig" class:big>{@render figure()}</p>{/if}
	{#if note && !empty}<div class="note">{@render note()}</div>{/if}
	{#if empty}
		<p class="empty">{empty}</p>
	{:else}
		<p class="readout num" aria-live="polite">{readout || 'Tap a day to see its values.'}</p>
		<div class="plot" role="img" aria-label={summary}>{@render children()}</div>
		{#if legend}<p class="legend">{@render legend()}</p>{/if}
	{/if}
</section>

<style>
	.chart-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.fig {
		font-size: 1.75rem;
	}
	.fig.big {
		font-size: 2.5rem;
	}
	.note {
		margin: calc(var(--space-1) * -1) 0 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.note :global(strong) {
		color: var(--text);
		font-weight: var(--weight-bold);
	}
	.readout {
		min-height: 2.75em; /* two lines on a phone, so tapping never shifts the chart */
		margin: 0;
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
	.plot {
		height: 11.5rem;
		touch-action: pan-y;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	/* What the charts put inside the card: axis, legend keys and the shared bar motion. */
	.chart-card :global(.chart-wrap) {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	.chart-card :global(.chart) {
		flex: 1;
		width: 100%;
		min-height: 0;
		overflow: visible;
	}
	.chart-card :global(.axis) {
		display: flex;
		justify-content: space-between;
		padding-top: var(--space-1);
		font-size: var(--text-xs);
		color: var(--chart-axis);
	}
	/* Legend keys: a shape plus a word, so color is never the only signal. */
	.chart-card :global(.key) {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.chart-card :global(.swatch) {
		display: inline-block;
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 3px;
		background: var(--c, var(--accent));
	}
	.chart-card :global(.swatch.tick) {
		width: 1rem;
		height: 3px;
		border-radius: 0;
	}
	/* Bars grow from the baseline; SVG needs the transform box set for the helper's origin. */
	.chart-card :global(.bar) {
		transform-box: fill-box;
	}
	@media (prefers-reduced-motion: no-preference) {
		.chart-card :global(.grow-x) {
			transform-box: fill-box;
		}
	}
</style>
