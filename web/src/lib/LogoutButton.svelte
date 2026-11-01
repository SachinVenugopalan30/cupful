<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  LogoutButton — the account-page logout, with the queued-write confirmation. The avatar menu has
  its own menu-item version and shares the same logout flow.
-->
<script lang="ts">
	import { errorMessage } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { logoutWarning, pendingChanges, signOut } from '#lib/logout.ts';

	let error = $state('');
	let confirming = $state(false);
	let waiting = $state(0);
	let busy = $state(false);

	async function ask() {
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

{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}
{#if confirming}
	<div class="notice notice-warning" role="group" aria-label="Confirm logout">
		<p>{logoutWarning(waiting)}</p>
		<div class="row">
			<button class="btn btn-danger" onclick={logout} disabled={busy}>Log out and discard</button>
			<button class="btn btn-quiet" onclick={() => (confirming = false)} disabled={busy}>
				Cancel
			</button>
		</div>
	</div>
{:else}
	<p>
		<button class="btn btn-danger" onclick={ask} disabled={busy}>
			{busy ? 'Logging out…' : 'Log out'}
		</button>
	</p>
{/if}
