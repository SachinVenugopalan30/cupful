<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  SettingRow — the two-column settings row: labels and a short explanation on the left, controls in
  a Mineral card on the right. Stacked on phones. `anchorId` gives the row a fragment target;
  `focusable` lets a link move focus to it.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		headingId,
		title,
		description,
		anchorId = undefined,
		focusable = false,
		testId = undefined,
		children
	}: {
		headingId: string;
		title: string;
		description?: string;
		anchorId?: string;
		focusable?: boolean;
		testId?: string;
		children: Snippet;
	} = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<section
	id={anchorId}
	class="setting-row"
	aria-labelledby={headingId}
	data-testid={testId}
	tabindex={focusable ? -1 : undefined}
>
	<div class="setting-copy">
		<h2 id={headingId}>{title}</h2>
		{#if description}<p>{description}</p>{/if}
	</div>
	<div class="setting-panel">
		{@render children()}
	</div>
</section>

<style>
	.setting-row {
		display: grid;
		grid-template-columns: minmax(11rem, 0.65fr) minmax(0, 1.35fr);
		gap: var(--space-6);
		align-items: start;
		padding: var(--space-5);
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-card);
	}
	.setting-copy h2 {
		margin: 0 0 var(--space-2);
	}
	.setting-copy p {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-sm);
		line-height: var(--leading);
	}
	.setting-panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}
	@media (max-width: 47.99rem) {
		.setting-row {
			grid-template-columns: 1fr;
			gap: var(--space-4);
			padding: var(--space-4);
		}
	}
</style>
