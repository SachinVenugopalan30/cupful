<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ChartCard — the frame every Trends chart sits in: title, a screen-reader summary, the value of
  the day last tapped, an optional legend/footnote, and the empty state.
  Props:
    title: string       card heading
    summary: string     plain-words description of the data; the chart itself is hidden from
                        assistive tech (role="img") and this is its label
    readout: string     the tapped day's values; empty shows the tap hint
    empty: string       when set, replaces the chart with this sentence
    children (the chart), legend snippets
  Maps LayerChart's color variables onto the app tokens so every chart follows light and dark.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		summary,
		readout = '',
		empty = '',
		legend,
		note,
		children
	}: {
		title: string;
		summary: string;
		readout?: string;
		empty?: string;
		legend?: Snippet;
		note?: string;
		children: Snippet;
	} = $props();
	const id = $props.id();
</script>

<section class="card stack" aria-labelledby="{id}-h">
	<h2 id="{id}-h">{title}</h2>
	{#if empty}
		<p class="empty">{empty}</p>
	{:else}
		{#if legend}<p class="legend small">{@render legend()}</p>{/if}
		<p class="readout num" aria-live="polite">{readout || 'Tap a day to see its values.'}</p>
		<div class="plot" role="img" aria-label={summary}>{@render children()}</div>
		{#if note}<p class="hint">{note}</p>{/if}
	{/if}
</section>

<style>
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
		height: 13rem;
		/* LayerChart's color variables, mapped to the tokens. */
		--color-surface-content: var(--chart-axis);
		--color-surface-100: var(--surface);
		--color-surface-300: var(--chart-grid);
		--color-primary: var(--chart-intake);
		--color-danger: var(--chart-over);
		touch-action: pan-y;
	}
</style>
