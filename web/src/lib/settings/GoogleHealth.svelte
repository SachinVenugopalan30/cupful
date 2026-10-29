<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  GoogleHealth — Settings card: connect Google Health, last sync, Sync now, disconnect. Shown only
  when the server has a Google client configured (docs/google-health.md). No props.
-->
<script lang="ts">
	import { errorMessage, google, type GoogleStatus } from '#lib/api.ts';
	import { googleResult, googleView } from './google.ts';

	let status = $state<GoogleStatus | null>(null);
	let loadError = $state('');
	let message = $state('');
	let messageOk = $state(true);
	let busy = $state(false);

	const view = $derived(googleView(status));

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

	async function syncNow() {
		busy = true;
		message = '';
		try {
			const r = await google.sync();
			messageOk = true;
			message = `Synced ${r.days} ${r.days === 1 ? 'day' : 'days'} of steps.`;
		} catch (e) {
			messageOk = false;
			message = errorMessage(e);
		} finally {
			busy = false;
			await load();
		}
	}

	async function disconnect() {
		if (!confirm('Disconnect Google Health? Steps already synced stay; Cupful forgets the connection and asks Google to revoke it.')) return;
		busy = true;
		message = '';
		try {
			await google.disconnect();
			messageOk = true;
			message = 'Google Health disconnected.';
		} catch (e) {
			messageOk = false;
			message = errorMessage(e);
		} finally {
			busy = false;
			await load();
		}
	}
</script>

{#if loadError}
	<section class="card stack" aria-labelledby="google-h">
		<h2 id="google-h">Google Health</h2>
		<p class="notice notice-error" role="alert">{loadError}</p>
	</section>
{:else if view !== 'hidden'}
	<section class="card stack" aria-labelledby="google-h">
		<h2 id="google-h">Google Health</h2>
		<p class="small muted">
			Reads your daily steps from Google Health into this server. Steps you enter by hand are never
			overwritten. Nothing is sent to Google except the sign-in and the steps request.
		</p>
		{#if message}
			<p class="notice {messageOk ? 'notice-info' : 'notice-error'}" role="status">{message}</p>
		{/if}

		{#if view === 'connect'}
			<p><a class="btn btn-primary" href={google.connectUrl}>Connect Google Health</a></p>
		{:else if status?.configured && status.connected}
			{#if view === 'reconnect'}
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
				<div class="row">
					<button class="btn" onclick={syncNow} disabled={busy}>
						{busy ? 'Syncing…' : 'Sync now'}
					</button>
					<button class="btn btn-quiet" onclick={disconnect} disabled={busy}>Disconnect</button>
				</div>
			{/if}
			{#if view === 'reconnect'}
				<p><button class="btn btn-quiet" onclick={disconnect} disabled={busy}>Disconnect</button></p>
			{/if}
		{/if}
	</section>
{/if}
