<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ChartCard — the frame every Trends chart sits in: title and headline figure, a screen-reader
  summary, the value of the day last tapped, legend and footnote snippets, and the empty state.
  The chart itself is presentational; this card carries `role="img"` and the summary text.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		summary,
		readout = '',
		empty = '',
		figure,
		legend,
		note,
		children
	}: {
		title: string;
		summary: string;
		readout?: string;
		empty?: string;
		figure?: Snippet;
		legend?: Snippet;
		note?: Snippet;
		children: Snippet;
	} = $props();
	const id = $props.id();
</script>

<section class="card stack" aria-labelledby="{id}-h">
	<div class="head">
		<h2 id="{id}-h">{title}</h2>
		{#if figure}<div class="figure">{@render figure()}</div>{/if}
	</div>
	{#if empty}
		<p class="empty">{empty}</p>
	{:else}
		{#if legend}<p class="legend small">{@render legend()}</p>{/if}
		<p class="readout num" aria-live="polite">{readout || 'Tap a day to see its values.'}</p>
		<div class="plot" role="img" aria-label={summary}>{@render children()}</div>
		{#if note}<div class="note">{@render note()}</div>{/if}
	{/if}
</section>

<style>
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
	}
	h2 {
		margin: 0;
	}
	.figure {
		font-size: var(--text-lg);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		white-space: nowrap;
	}
	.figure :global(small) {
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
		color: var(--text-muted);
	}
	.readout {
		min-height: 2.75em; /* two lines on a phone, so tapping never shifts the chart */
		margin: 0;
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		color: var(--text-muted);
	}
	.plot {
		height: 11.5rem;
		touch-action: pan-y;
	}
	.note {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	.note :global(strong) {
		color: var(--text);
		font-weight: var(--weight-bold);
	}
</style>
