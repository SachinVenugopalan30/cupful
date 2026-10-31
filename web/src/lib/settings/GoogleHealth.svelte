<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  GoogleHealth — Settings card: connect Google Health, last sync, Sync now, disconnect. Shown only
  when the server has a Google client configured (docs/google-health.md). No props.
-->
<script lang="ts">
	import { errorMessage, google, type GoogleStatus } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { googleResult } from './google.ts';
	import SettingRow from './SettingRow.svelte';

	let status = $state<GoogleStatus | null>(null);
	let loadError = $state('');
	let message = $state('');
	let messageOk = $state(true);
	let busy = $state(false);

	async function load() {
		try {
			status = await google.status();
		} catch (e) {
			loadError = errorMessage(e);
		}
	}
	$effect(() => {
		const r = googleResult(location.search);
		if (r) {
			({ ok: messageOk, message } = r);
			// Drop ?google= so a reload doesn't repeat the message.
			history.replaceState(null, '', location.pathname);
		}
		load();
	});

	/** Runs `fn`, shows the message it returns (or the error), then reloads the status. */
	async function run(fn: () => Promise<string>) {
		busy = true;
		message = '';
		try {
			message = await fn();
			messageOk = true;
		} catch (e) {
			messageOk = false;
			message = errorMessage(e);
		} finally {
			busy = false;
			await load();
		}
	}

	const syncNow = () =>
		run(async () => {
			const r = await google.sync();
			return `Synced ${r.days} ${r.days === 1 ? 'day' : 'days'} of steps.`;
		});

	function disconnect() {
		if (!confirm('Disconnect Google Health? Steps already synced stay; Cupful forgets the connection and asks Google to revoke it.')) return;
		run(async () => {
			await google.disconnect();
			return 'Google Health disconnected.';
		});
	}
</script>

{#if loadError}
	<SettingRow headingId="google-h" title="Google Health" description="Reads your daily steps into this server.">
		<p class="notice notice-error" role="alert">{loadError}</p>
	</SettingRow>
{:else if status?.configured}
	<SettingRow headingId="google-h" title="Google Health" description="Reads your daily steps into this server.">
		<p class="small muted">
			Reads your daily steps from Google Health into this server. Steps you enter by hand are never
			overwritten. Nothing is sent to Google except the sign-in and the steps request.
		</p>
		{#if !connection.online}
			<p class="notice notice-warning">Settings need a connection. Google Health changes need a connection.</p>
		{/if}
		{#if message}
			<p class="notice {messageOk ? 'notice-info' : 'notice-error'}" role="status">{message}</p>
		{/if}

		{#if !status.connected}
			<p><a class="btn btn-primary" href={google.connectUrl}>Connect Google Health</a></p>
		{:else}
			{#if status.needs_reconnect}
				<p class="notice notice-warning" role="alert">
					Google Health needs to be connected again. {status.last_error ?? ''}
				</p>
				<p><a class="btn btn-primary" href={google.connectUrl}>Reconnect Google Health</a></p>
			{:else}
				<p class="small" aria-live="polite">
					Last sync:
					{status.last_sync_at ? new Date(status.last_sync_at).toLocaleString() : 'not yet'}
				</p>
				{#if status.last_error}<p class="notice notice-error" role="alert">{status.last_error}</p>{/if}
			{/if}
			<div class="row">
				{#if !status.needs_reconnect}
					<button class="btn" onclick={syncNow} disabled={busy || !connection.online}>
						{busy ? 'Syncing…' : 'Sync now'}
					</button>
				{/if}
				<button class="btn btn-quiet" onclick={disconnect} disabled={busy || !connection.online}>Disconnect</button>
			</div>
		{/if}
	</SettingRow>
{/if}
