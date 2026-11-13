<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Jug — the day's budget as a graduated measuring jug. The liquid rises by `fraction` (0 to 1) and
  pours over --dur-pour when it changes; it renders at its level on first paint, with no pour.
  The jug outline carries the shape; the figures beside it carry the meaning in text.
  Props:
    fraction: number   0 to 1, from jugFraction()
    marks: JugMark[]   graduations, top first; none draws a bare jug
    label: string      the accessible name (eaten and plan)
-->
<script lang="ts">
	import type { JugMark } from './jug.ts';

	let { fraction, marks = [], label }: { fraction: number; marks?: JugMark[]; label: string } =
		$props();
</script>

<div class="jug" role="img" aria-label={label}>
	<div class="liquid" style="--v: {fraction}"></div>
	<ol class="grads" aria-hidden="true">
		{#each marks as m (m.at)}
			<li style="--at: {m.at}"><span>{m.label}</span></li>
		{/each}
	</ol>
</div>

<style>
	.jug {
		position: relative;
		block-size: var(--jug-h, 14.75rem);
		border: 3px solid var(--text);
		border-block-start: 0;
		border-radius: 6px 6px 26px 26px;
		background: var(--glass);
		overflow: hidden;
	}
	/* The pour lip: the rim with a gap, like a spout. */
	.jug::before {
		content: '';
		position: absolute;
		inset-block-start: 0;
		inset-inline: -3px;
		block-size: 3px;
		background: linear-gradient(
			90deg,
			var(--text) 0 14px,
			transparent 14px calc(100% - 14px),
			var(--text) 0
		);
		z-index: 2;
	}
	/*
	 * A full-height block slid down by the empty share. Translating keeps the meniscus the same
	 * thickness at every level; scaling would squash it.
	 */
	.liquid {
		position: absolute;
		inset: 0;
		background: var(--fill);
		box-shadow: inset 0 6px 0 var(--fill-top);
		transform: translateY(calc((1 - var(--v, 0)) * 100%));
		transition: transform var(--dur-pour) var(--ease);
	}
	.grads {
		position: absolute;
		inset: 0;
		margin: 0;
		padding: 0;
		list-style: none;
		z-index: 1;
	}
	.grads li {
		position: absolute;
		inset-block-start: calc((1 - var(--at)) * 100%);
		inline-size: 52%;
		border-block-start: 2px solid color-mix(in oklab, var(--text) 70%, transparent);
		font-size: 0.6875rem;
		font-weight: 700;
		line-height: 1.2;
		font-variant-numeric: tabular-nums;
		color: var(--text);
	}
	.grads li:first-child {
		border-block-start-style: dashed;
	}
	/* The labels sit on a small surface chip so they read over the glass and over the liquid. */
	.grads span {
		display: inline-block;
		margin: 2px 0 0 4px;
		padding: 0 4px;
		border-radius: 4px;
		background: var(--surface);
	}
</style>
