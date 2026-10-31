<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  SyncBar — compact status above the page content: offline, waiting changes, changes that need
  attention, or a paused queue (re-login / another account's changes). Hidden when everything is
  in order. A slim neutral bar, not a warning block. No props; reads the shared offline state.
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
	const barState = $derived.by((): BarState => {
		if (connection.paused === 'identity') return 'identity';
		if (connection.paused === 'auth') return 'signin';
		if (offline) return 'offline';
		if (attention) return 'attention';
		return 'pending';
	});
</script>

{#if show}
	<div class="sync-bar" role="status" data-testid="sync-bar" data-state={barState}>
		<svg class="bar-icon" viewBox="0 0 24 24" aria-hidden="true">
			<path d="M20 11a8 8 0 0 0-14.9-3M4 4v4h4M4 13a8 8 0 0 0 14.9 3M20 20v-4h-4" />
		</svg>
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
			<button class="bar-action" onclick={sync} disabled={busy}>
				{busy ? 'Syncing…' : 'Sync now'}
			</button>
		{/if}
	</div>
{/if}

<style>
	.sync-bar {
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
	.sync-bar p {
		flex: 1 1 12rem;
		min-width: 0;
		margin: 0;
	}
	.sync-bar a {
		color: var(--accent);
		font-weight: var(--weight-medium);
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
	.bar-action:disabled {
		opacity: 0.6;
		cursor: default;
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
