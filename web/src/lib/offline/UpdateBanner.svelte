<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UpdateBanner — shown when a new service-worker version is waiting. Reload hands control to the
  waiting worker; the page reloads once it controls the tab.
-->
<script lang="ts">
	import { applyUpdate, updateState } from './update.svelte.ts';
</script>

{#if updateState.available}
	<div class="notice notice-info banner" role="status" data-testid="update-banner">
		<p>A new version is available.</p>
		<button class="btn" onclick={applyUpdate}>Reload</button>
	</div>
{/if}

<style>
	.banner {
		flex-direction: row;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.banner p {
		margin: 0;
	}
	.banner .btn {
		min-height: 2.25rem;
		padding: 0 var(--space-3);
	}
</style>
