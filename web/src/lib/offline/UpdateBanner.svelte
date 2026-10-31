<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UpdateBanner — shown when a new service-worker version is waiting. Reload hands control to the
  waiting worker; the page reloads once it controls the tab. Uses the same slim bar as SyncBar.
-->
<script lang="ts">
	import { applyUpdate, updateState } from './update.svelte.ts';
</script>

{#if updateState.available}
	<div class="update-bar" role="status" data-testid="update-banner">
		<svg class="bar-icon" viewBox="0 0 24 24" aria-hidden="true">
			<path d="M12 4v10M8 10l4 4 4-4M5 20h14" />
		</svg>
		<p>A new version is available.</p>
		<button class="bar-action" onclick={applyUpdate}>Reload</button>
	</div>
{/if}

<style>
	.update-bar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
		padding: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
		background: var(--surface-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		color: var(--text);
		font-size: var(--text-sm);
		animation: bar-in var(--dur-fast) var(--ease);
	}
	.bar-icon {
		flex: none;
		width: 1rem;
		height: 1rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.75;
		stroke-linecap: round;
		stroke-linejoin: round;
		color: var(--text-muted);
	}
	.update-bar p {
		flex: 1 1 12rem;
		min-width: 0;
		margin: 0;
	}
	.bar-action {
		margin-left: auto;
		min-height: var(--touch);
		padding: 0 var(--space-3);
		border: 1px solid var(--control-border);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-weight: var(--weight-medium);
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease),
			transform var(--dur-fast) var(--ease);
	}
	.bar-action:hover {
		background: var(--surface-2);
	}
	.bar-action:active {
		transform: scale(0.98);
	}
	.bar-action:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	@keyframes bar-in {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
	}
</style>
