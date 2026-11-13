<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  SettingRow — one settings section. On the Settings page it is a <details>: the summary shows the
  title and a one-line description (the caller passes live state, such as "All changes synced.", when
  it already has it), and opening it reveals the controls. `flat` is the plain card for a page that
  is the section (Profile). `anchorId` gives the row a fragment target; `focusable` lets a link
  move focus to it. The Settings page opens a row when the URL hash names it.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		headingId,
		title,
		description,
		anchorId = undefined,
		focusable = false,
		flat = false,
		open = false,
		testId = undefined,
		children
	}: {
		headingId: string;
		title: string;
		description?: string;
		anchorId?: string;
		focusable?: boolean;
		flat?: boolean;
		/** Start open (a returning Google redirect, say). The page also opens rows by URL hash. */
		open?: boolean;
		testId?: string;
		children: Snippet;
	} = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<section
	id={anchorId}
	class="setting-row"
	class:flat
	aria-labelledby={headingId}
	data-testid={testId}
	tabindex={focusable ? -1 : undefined}
>
	{#if flat}
		<div class="copy">
			<h2 id={headingId}>{title}</h2>
			{#if description}<p>{description}</p>{/if}
		</div>
		<div class="body">{@render children()}</div>
	{:else}
		<details {open}>
			<summary>
				<div class="copy">
					<h2 id={headingId}>{title}</h2>
					{#if description}<p>{description}</p>{/if}
				</div>
				<svg class="chev" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
			</summary>
			<div class="body">{@render children()}</div>
		</details>
	{/if}
</section>

<style>
	.copy h2 {
		margin: 0;
		font-size: var(--text-md);
	}
	.copy p {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-sm);
		line-height: var(--leading);
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
		padding-top: var(--space-1);
		padding-bottom: var(--space-4);
	}

	/* Phone: a row inside a grouped list (the page draws the group's frame and dividers). */
	summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		min-height: 3.5rem;
		padding: var(--space-2) var(--space-4);
		cursor: pointer;
		list-style: none;
	}
	summary::-webkit-details-marker {
		display: none;
	}
	summary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -2px;
		border-radius: var(--radius-lg);
	}
	.chev {
		flex: none;
		inline-size: 1.25rem;
		block-size: 1.25rem;
		fill: none;
		stroke: var(--text-muted);
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	@media (prefers-reduced-motion: no-preference) {
		.chev {
			transition: rotate var(--dur-med) var(--ease);
		}
	}
	details[open] > summary .chev {
		rotate: 90deg;
	}
	details > .body {
		padding-inline: var(--space-4);
	}

	/* The plain card: Profile page. */
	.flat {
		padding: var(--space-4);
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.flat .body {
		padding: 0;
	}
	.flat .copy h2 {
		font-size: 1.125rem;
	}

	/* Desktop: every row is its own card, open; the summary is a plain header. */
	@media (min-width: 60rem) {
		.setting-row:not(.flat) {
			background: var(--surface);
			border: 1px solid var(--border);
			border-radius: var(--radius-lg);
		}
		summary {
			cursor: default;
			padding: var(--space-4) var(--space-5) var(--space-3);
		}
		.chev {
			display: none;
		}
		details > .body {
			padding: 0 var(--space-5) var(--space-5);
		}
		.copy h2 {
			font-size: 1.125rem;
		}
	}
</style>
