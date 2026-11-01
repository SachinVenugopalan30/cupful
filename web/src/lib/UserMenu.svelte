<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UserMenu — the avatar in the wide top bar. Opens a small account menu with Profile and Log out.
  Hidden below 48 rem, where Settings links to the profile page instead.
-->
<script lang="ts">
	import { errorMessage } from '#lib/api.ts';
	import { dismiss } from '#lib/dismiss.ts';
	import { logoutWarning, pendingChanges, signOut } from '#lib/logout.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { session } from '#lib/session.svelte.ts';

	const initial = $derived(session.user?.username.slice(0, 1).toUpperCase() ?? '?');
	const username = $derived(session.user?.username ?? '');

	let error = $state('');
	let confirming = $state(false);
	let waiting = $state(0);
	let busy = $state(false);

	async function askLogout() {
		error = '';
		if (!connection.online) {
			error = 'Logging out needs a connection. Reconnect so queued changes can sync first.';
			return;
		}
		waiting = await pendingChanges();
		if (waiting > 0) {
			confirming = true;
			return;
		}
		await logout();
	}

	async function logout() {
		busy = true;
		error = '';
		try {
			await signOut();
		} catch (e) {
			error = errorMessage(e);
			confirming = false;
		} finally {
			busy = false;
		}
	}
</script>

<details class="menu user-menu" use:dismiss>
	<summary class="user-chip" aria-label="Account menu for {username}">
		<span aria-hidden="true">{initial}</span>
	</summary>
	<div class="menu-panel account-panel" role="group" aria-label="Account menu">
		<div class="account-head">
			<p class="account-name">{username}</p>
			<p class="small muted">Signed in</p>
		</div>
		<a class="menu-item" href="/profile" data-close>Profile</a>
		{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}
		{#if confirming}
			<div class="notice notice-warning" role="group" aria-label="Confirm logout">
				<p>{logoutWarning(waiting)}</p>
				<div class="row">
					<button class="btn btn-danger" onclick={logout} disabled={busy}>
						Log out and discard
					</button>
					<button class="btn btn-quiet" onclick={() => (confirming = false)} disabled={busy}>
						Cancel
					</button>
				</div>
			</div>
		{:else}
			<button class="menu-item danger" onclick={askLogout} disabled={busy}>
				{busy ? 'Logging out…' : 'Log out'}
			</button>
		{/if}
	</div>
</details>

<style>
	.user-menu {
		display: none;
		margin-left: auto;
	}
	.user-chip {
		display: grid;
		place-items: center;
		width: 2.25rem;
		height: 2.25rem;
		border: 0;
		border-radius: 50%;
		background: var(--accent-soft);
		color: var(--accent);
		font: inherit;
		font-size: var(--text-sm);
		font-weight: var(--weight-bold);
		cursor: pointer;
		list-style: none;
	}
	.user-chip::-webkit-details-marker {
		display: none;
	}
	.user-chip:hover {
		background: var(--surface-2);
	}
	.user-chip:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.account-panel {
		left: auto;
		right: 0;
		width: 18rem;
		padding: var(--space-2);
	}
	.account-head {
		padding: var(--space-2) var(--space-3) var(--space-3);
		margin-bottom: var(--space-1);
		border-bottom: 1px solid var(--border);
	}
	.account-name {
		margin: 0;
		font-weight: var(--weight-bold);
		overflow-wrap: anywhere;
	}
	.menu-item {
		display: flex;
		align-items: center;
		width: 100%;
		min-height: var(--touch);
		padding: 0 var(--space-3);
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text);
		font: inherit;
		text-align: left;
		text-decoration: none;
		cursor: pointer;
	}
	.menu-item:hover {
		background: var(--surface-2);
	}
	.menu-item.danger {
		color: var(--error);
	}
	.menu-item:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
	@media (min-width: 48rem) {
		.user-menu {
			display: block;
		}
	}
</style>
