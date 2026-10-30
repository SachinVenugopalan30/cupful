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
	const waiting = $derived(
		`${connection.pending} ${connection.pending === 1 ? 'change' : 'changes'} waiting`
	);

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
		class="notice notice-bar"
		class:notice-warning={barState === 'identity' || barState === 'signin' || barState === 'attention'}
		class:notice-info={barState === 'pending'}
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
				{connection.pending > 0 ? ` ${waiting}.` : ''}
				<a href="/login" data-sveltekit-reload>Sign in</a>
			</p>
		{:else if barState === 'offline'}
			<p>
				{connection.pending > 0
					? `Offline. ${waiting} to sync.`
					: 'Offline. Changes sync when you reconnect.'}
			</p>
		{:else if barState === 'attention'}
			<p>
				<a href="/settings#sync">{connection.failed} changes need attention</a>
			</p>
		{:else}
			<p>{waiting}</p>
			<button class="btn" onclick={sync} disabled={busy}>
				{busy ? 'Syncing…' : 'Sync now'}
			</button>
		{/if}
	</div>
{/if}
