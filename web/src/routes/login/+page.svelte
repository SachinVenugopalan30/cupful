<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { ApiError, auth, errorMessage, health, type Health } from '#lib/api.ts';
	import Wordmark from '#lib/Wordmark.svelte';
	import { loadSession, session } from '#lib/session.svelte.ts';
	import { clearUser, countFor } from '#lib/offline/queue.ts';
	import { clearPreviousUser, loadPreviousUser } from '#lib/offline/session-cache.ts';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);
	/** Running version and AGPL source offer. Absent when the server cannot be reached. */
	let server = $state<Health | null>(null);
	/** A previous account with unsynced changes; logging in as someone else would lose them. */
	let switchFrom = $state<{ userId: number; username: string; waiting: number } | null>(null);
	const switchWarning = $derived.by(() => {
		if (!switchFrom) return '';
		const n = switchFrom.waiting;
		return `${n} ${n === 1 ? 'change' : 'changes'} from ${switchFrom.username} ${
			n === 1 ? 'hasn’t' : 'haven’t'
		} synced. Logging in as ${username} discards ${n === 1 ? 'it' : 'them'}.`;
	});

	onMount(() => {
		let active = true;
		health()
			.then((h) => {
				if (active) server = h;
			})
			.catch(() => {});
		return () => {
			active = false;
		};
	});

	async function login(discard = false) {
		busy = true;
		error = '';
		try {
			const previous = await loadPreviousUser().catch(() => null);
			const target = username.trim().toLowerCase();
			if (
				!discard &&
				previous &&
				previous.username.toLowerCase() !== target
			) {
				const waiting = await countFor(previous.userId).catch(() => 0);
				if (waiting > 0) {
					switchFrom = { userId: previous.userId, username: previous.username, waiting };
					return;
				}
			}
			await auth.login({ username, password });
			await loadSession();
			if (previous && session.user && previous.userId !== session.user.id) {
				await clearUser(previous.userId).catch(() => {});
			}
			await clearPreviousUser().catch(() => {});
			await goto('/', { replaceState: true });
		} catch (err) {
			error =
				err instanceof ApiError && err.status === 401
					? 'Wrong username or password.'
					: err instanceof ApiError && err.status === 429
						? 'Too many login attempts. Wait a minute, then try again.'
						: errorMessage(err);
		} finally {
			busy = false;
		}
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		void login();
	}
</script>

<svelte:head><title>Log in · Cupful</title></svelte:head>

<div class="auth-page">
	<div class="auth-card">
		<Wordmark height="2rem" />
		<div class="auth-intro">
			<h1>Log in</h1>
			<p class="muted">Your food, activity and trends, on your own server.</p>
		</div>
		<form class="stack" onsubmit={submit}>
			<div class="field">
				<label for="username">Username</label>
				<input id="username" bind:value={username} autocomplete="username" autocapitalize="none" required />
			</div>
			<div class="field">
				<label for="password">Password</label>
				<input id="password" type="password" bind:value={password} autocomplete="current-password" required />
			</div>
			{#if switchFrom}
				<div class="notice notice-warning" role="group" aria-label="Confirm account switch">
					<p>{switchWarning}</p>
					<div class="row">
						<button class="btn btn-danger" type="button" onclick={() => login(true)} disabled={busy}>
							Log in and discard
						</button>
						<button class="btn btn-quiet" type="button" onclick={() => (switchFrom = null)} disabled={busy}>
							Cancel
						</button>
					</div>
				</div>
			{/if}
			{#if error}
				<p class="notice notice-error" role="alert">{error}</p>
			{/if}
			<button class="btn btn-primary btn-block" disabled={busy}>{busy ? 'Logging in…' : 'Log in'}</button>
		</form>
		<p class="muted small auth-meta">
			Forgot your password? The server admin can reset it with <code>cupful reset-password</code>.
		</p>
		{#if server}
			<p class="muted small auth-meta">
				Cupful <span class="num">{server.version}</span> ·
				<a href={server.source_url} rel="noopener">Source code</a> (AGPL-3.0-or-later)
			</p>
		{:else}
			<p class="muted small auth-meta">Free software under AGPL-3.0-or-later.</p>
		{/if}
	</div>
</div>

<style>
	.auth-page {
		display: grid;
		place-items: center;
		min-height: calc(100dvh - var(--space-8));
		padding: var(--space-4) 0;
	}
	.auth-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		width: min(26rem, 100%);
		padding: var(--space-6) var(--space-5);
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-card);
	}
	.auth-intro {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.auth-intro h1,
	.auth-intro p {
		margin: 0;
	}
	.auth-card form {
		gap: var(--space-4);
	}
	.auth-meta {
		margin: 0;
		text-align: center;
	}
	input {
		transition:
			border-color var(--dur-fast) var(--ease),
			background var(--dur-fast) var(--ease);
	}
</style>
