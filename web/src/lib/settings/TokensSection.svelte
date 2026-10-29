<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  TokensSection — ingest tokens for iOS Shortcuts and other automations (PLAN.md §11): list,
  create (the token is shown once), revoke after a confirmation. Config REST, online only.
  The create reply is the only time the server returns the token itself.
-->
<script lang="ts">
	import { errorMessage, ingestTokens, type IngestToken } from '#lib/api.ts';
	import { today } from '#lib/session.svelte.ts';

	let list = $state<IngestToken[] | null>(null);
	let listError = $state('');
	let label = $state('');
	let formError = $state('');
	let busy = $state(false);
	let status = $state('');
	/** The token just created, shown once. */
	let fresh = $state<{ label: string; token: string } | null>(null);
	let copied = $state(false);
	let confirming = $state<number | null>(null);

	async function load() {
		listError = '';
		try {
			list = await ingestTokens.list();
		} catch (e) {
			listError = errorMessage(e);
		}
	}
	load();

	async function create(e: SubmitEvent) {
		e.preventDefault();
		formError = status = '';
		if (!label.trim()) return void (formError = 'Give the token a name, like "iPhone".');
		busy = true;
		try {
			const t = await ingestTokens.create(label.trim());
			fresh = { label: t.label, token: t.token };
			copied = false;
			label = '';
			await load();
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function copy(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = true;
		} catch {
			copied = false;
			formError = "Couldn't copy. Select the token and copy it by hand.";
		}
	}

	async function revoke(t: IngestToken) {
		status = '';
		try {
			await ingestTokens.remove(t.id);
			status = `Revoked ${t.label}.`;
			confirming = null;
			await load();
		} catch (err) {
			listError = errorMessage(err);
		}
	}

	const when = (iso: string) => new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
</script>

<section class="card stack" aria-labelledby="tokens-h">
	<h2 id="tokens-h">Ingest tokens</h2>
	<p class="small muted">
		A token lets an automation, such as an iOS Shortcut, post your steps and gym minutes. It can't
		read anything or overwrite a value you typed yourself. Setup steps are in
		<code>docs/shortcuts.md</code> in the source code.
	</p>
	{#if listError}<p class="notice notice-error" role="alert">{listError}</p>{/if}
	<p class="status small" role="status">{status}</p>

	{#if list === null && !listError}
		<div class="skeleton" style="height: 3rem" aria-busy="true"></div>
	{:else if list?.length === 0}
		<p class="empty">No tokens yet. Create one for each device or automation.</p>
	{:else if list}
		<ul class="list">
			{#each list as t (t.id)}
				<li class="token stack">
					<div class="stack head">
						<strong>{t.label}</strong>
						<span class="small muted">
							Created {when(t.created_at)} ·
							{t.last_used_at ? `last used ${when(t.last_used_at)}` : 'never used'}
						</span>
					</div>
					{#if confirming === t.id}
						<div class="notice notice-warning" role="group" aria-label="Confirm revoke">
							<p>Revoke “{t.label}”? Automations using it stop working.</p>
							<div class="row">
								<button class="btn btn-danger" onclick={() => revoke(t)}>Revoke token</button>
								<button class="btn btn-quiet" onclick={() => (confirming = null)}>Keep it</button>
							</div>
						</div>
					{:else}
						<div class="row">
							<button class="btn btn-quiet danger" onclick={() => (confirming = t.id)} aria-label="Revoke {t.label}">Revoke</button>
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	{#if fresh}
		<div class="notice notice-info fresh stack" role="group" aria-label="New token">
			<p><strong>Copy this token now.</strong> It's shown once; after that only its name remains.</p>
			<p><code class="secret" data-testid="new-token">{fresh.token}</code></p>
			<div class="row">
				<button class="btn" onclick={() => copy(fresh!.token)}>{copied ? 'Copied' : 'Copy token'}</button>
				<button class="btn btn-quiet" onclick={() => (fresh = null)}>Done</button>
			</div>
			<details>
				<summary>Test it with curl</summary>
				<pre class="curl">curl -X POST {location.origin}/api/ingest/activity \
  -H 'Authorization: Bearer {fresh.token}' \
  -H 'Content-Type: application/json' \
  -d '{`{"date":"${today()}","steps":5000}`}'</pre>
			</details>
		</div>
	{/if}

	<form class="stack" onsubmit={create}>
		<div class="field">
			<label for="token-label">New token name</label>
			<input id="token-label" bind:value={label} maxlength="100" placeholder="iPhone Shortcut" />
		</div>
		{#if formError}<p class="notice notice-error" role="alert">{formError}</p>{/if}
		<p><button class="btn btn-primary" disabled={busy}>Create token</button></p>
	</form>
</section>

<style>
	.token {
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
	.curl {
		margin: var(--space-2) 0 0;
		padding: var(--space-3);
		overflow-x: auto;
		border-radius: var(--radius-sm);
		background: var(--surface);
		font-size: var(--text-xs);
	}
	summary {
		min-height: var(--touch);
		display: flex;
		align-items: center;
		cursor: pointer;
	}
</style>
