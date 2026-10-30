<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UpdateBanner — shown when a new service-worker version is waiting. Reload hands control to the
  waiting worker; the page reloads once it controls the tab.
-->
<script lang="ts">
	import { applyUpdate, updateState } from './update.svelte.ts';
</script>

{#if updateState.available}
	<div class="notice notice-info notice-bar" role="status" data-testid="update-banner">
		<p>A new version is available.</p>
		<button class="btn" onclick={applyUpdate}>Reload</button>
	</div>
{/if}
