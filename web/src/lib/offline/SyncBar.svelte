<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  SyncBar — compact status above the page content: offline, waiting changes, changes that need
  attention, or a paused queue (re-login / another account's changes). Hidden when everything is
  in order. No props; reads the shared offline state.
-->
<script lang="ts">
	import { connection } from './state.svelte.ts';
	import { syncNow } from './queue.ts';

	let busy = $state(false);

	async function sync() {
		busy = true;
		try {
			await syncNow();
		} finally {
			busy = false;
		}
	}

	/** `auth` needs a new sign-in; `identity` means the queue belongs to another account. */
	const paused = $derived(connection.paused === 'auth' || connection.paused === 'identity');
	const offline = $derived(!connection.online || connection.paused === 'offline');
	const attention = $derived(connection.failed > 0);
	const show = $derived(offline || paused || attention || connection.pending > 0);

	type BarState = 'offline' | 'signin' | 'identity' | 'attention' | 'pending';
	const barState = $derived<BarState>(
		connection.paused === 'identity'
			? 'identity'
			: connection.paused === 'auth'
				? 'signin'
				: offline
					? 'offline'
					: attention
						? 'attention'
						: 'pending'
	);
</script>

{#if show}
	<div
		class="notice bar"
		class:notice-warning={barState === 'offline' || barState === 'identity'}
		class:notice-info={barState === 'pending'}
		class:notice-error={barState === 'signin' || barState === 'attention'}
		role="status"
		data-testid="sync-bar"
		data-state={barState}
	>
		{#if barState === 'identity'}
			<p>
				Changes on this device belong to another account.
				<a href="/settings#sync">See the unsynced changes</a>.
			</p>
		{:else if barState === 'signin'}
			<p>
				Sign in again to sync your changes.
				<a href="/login">Sign in</a>
				{connection.pending > 0 ? `· ${connection.pending} waiting` : ''}
			</p>
		{:else if barState === 'offline'}
			<p>
				Offline. Changes will sync when you reconnect.
				{connection.pending > 0 ? `· ${connection.pending} waiting` : ''}
			</p>
		{:else if barState === 'attention'}
			<p>
				<a href="/settings#sync">{connection.failed} changes need attention</a>
			</p>
		{:else}
			<p>{connection.pending} waiting to sync</p>
			<button class="btn" onclick={sync} disabled={busy}>
				{busy ? 'Syncing…' : 'Sync now'}
			</button>
		{/if}
	</div>
{/if}

<style>
	.bar {
		flex-direction: row;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.bar p {
		margin: 0;
	}
	.bar .btn {
		min-height: 2.25rem;
		padding: 0 var(--space-3);
	}
</style>
