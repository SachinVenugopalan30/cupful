<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  UsersSection — invites for a password instance (PLAN.md §6). Admin only: create a one-time
  link, copy it, see pending invites and revoke one in the card. Hidden when the server signs
  people in with Tailscale. Config REST, read-only offline.
-->
<script lang="ts">
	import { ApiError, errorMessage, invites, type Invite } from '#lib/api.ts';
	import { inviteUrl } from '#lib/invites.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { session } from '#lib/session.svelte.ts';

	let { isAdmin }: { isAdmin: boolean } = $props();

	let list = $state<Invite[] | null>(null);
	let listError = $state('');
	let status = $state('');
	let busy = $state(false);
	/** The link just created, shown once. */
	let fresh = $state<{ url: string; expires_at: string } | null>(null);
	let copied = $state(false);
	let confirming = $state<number | null>(null);

	const identityMode = $derived(session.auth?.identity_mode === 'tailscale');

	async function load() {
		listError = '';
		try {
			list = await invites.list();
		} catch (e) {
			listError = errorMessage(e);
		}
	}

	$effect(() => {
		if (isAdmin && !identityMode) load();
	});

	async function create() {
		status = listError = '';
		busy = true;
		try {
			const made = await invites.create();
			fresh = { url: inviteUrl(location.origin, made.token), expires_at: made.expires_at };
			copied = false;
			await load();
		} catch (e) {
			listError = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	async function copy(url: string) {
		try {
			await navigator.clipboard.writeText(url);
			copied = true;
		} catch {
			copied = false;
			listError = "Couldn't copy. Select the link and copy it by hand.";
		}
	}

	async function revoke(id: number) {
		status = listError = '';
		try {
			await invites.remove(id);
			status = 'Invite revoked.';
		} catch (e) {
			listError =
				e instanceof ApiError && e.status === 404
					? 'That invite was already used or revoked.'
					: errorMessage(e);
		} finally {
			confirming = null;
			await load();
		}
	}

	const when = (iso: string) =>
		new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
</script>

{#if isAdmin}
	<section class="card stack" aria-labelledby="users-h">
		<h2 id="users-h">Users</h2>
		{#if identityMode}
			<p class="muted">
				This server signs people in with Tailscale. Anyone in your tailnet gets an account when
				they connect, so invites are off.
			</p>
		{:else}
			<p class="small muted">
				An invite lets one person create an account. Each link works once and expires after 7
				days. A used link stays used even if the account is later removed.
			</p>
			{#if !connection.online}
				<p class="notice notice-warning">
					Settings need a connection. Creating and revoking invites is disabled offline.
				</p>
			{/if}
			{#if listError}<p class="notice notice-error" role="alert">{listError}</p>{/if}
			<p class="status small" role="status">{status}</p>

			{#if list === null && !listError}
				<div class="skeleton" style="height: 3rem" aria-busy="true"></div>
			{:else if list?.length === 0}
				<p class="empty">No pending invites. Create one to let someone join.</p>
			{:else if list}
				<ul class="list">
					{#each list as invite (invite.id)}
						<li class="invite stack">
							<div class="stack head">
								<strong>Invite</strong>
								<span class="small muted">
									Created {when(invite.created_at)} · expires {when(invite.expires_at)}
								</span>
							</div>
							{#if confirming === invite.id}
								<div class="notice notice-warning" role="group" aria-label="Confirm revoke">
									<p>Revoke this invite? Its link stops working.</p>
									<div class="row">
										<button
											class="btn btn-danger"
											onclick={() => revoke(invite.id)}
											disabled={!connection.online}
										>
											Revoke invite
										</button>
										<button class="btn btn-quiet" onclick={() => (confirming = null)}>Keep it</button>
									</div>
								</div>
							{:else}
								<div class="row">
									<button
										class="btn btn-quiet danger"
										onclick={() => (confirming = invite.id)}
										disabled={!connection.online}
										aria-label="Revoke invite created {when(invite.created_at)}"
									>
										Revoke
									</button>
								</div>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}

			{#if fresh}
				<div class="notice notice-info fresh stack" role="group" aria-label="New invite link">
					<p>
						<strong>Copy this link now.</strong> It's shown once. The invite expires
						{when(fresh.expires_at)}.
					</p>
					<p><code class="secret" data-testid="new-invite">{fresh.url}</code></p>
					<div class="row">
						<button class="btn" onclick={() => copy(fresh!.url)}>{copied ? 'Copied' : 'Copy link'}</button>
						<button class="btn btn-quiet" onclick={() => (fresh = null)}>Done</button>
					</div>
				</div>
			{/if}

			<p>
				<button class="btn btn-primary" onclick={create} disabled={busy || !connection.online}>
					{busy ? 'Creating…' : 'Create invite'}
				</button>
			</p>
		{/if}
	</section>
{/if}

<style>
	.invite {
		flex-direction: column;
		align-items: stretch;
	}
	.secret {
		display: block;
		overflow-wrap: anywhere;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		background: var(--surface);
		user-select: all;
	}
	.status:empty {
		display: none;
	}
</style>
