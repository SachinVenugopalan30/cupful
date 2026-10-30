<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError, auth, errorMessage } from '#lib/api.ts';
	import { loadSession, session } from '#lib/session.svelte.ts';
	import { clearUser, countFor } from '#lib/offline/queue.ts';
	import { clearPreviousUser, loadPreviousUser } from '#lib/offline/session-cache.ts';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);
	/** A previous account with unsynced changes; logging in as someone else would lose them. */
	let switchFrom = $state<{ userId: number; username: string; waiting: number } | null>(null);
	const switchWarning = $derived.by(() => {
		if (!switchFrom) return '';
		const n = switchFrom.waiting;
		return `${n} ${n === 1 ? 'change' : 'changes'} from ${switchFrom.username} ${
			n === 1 ? 'hasn’t' : 'haven’t'
		} synced. Logging in as ${username} discards ${n === 1 ? 'it' : 'them'}.`;
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
	<p class="wordmark" aria-hidden="true">Cupful</p>
	<h1>Log in</h1>
	<form class="card stack" onsubmit={submit}>
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
	<p class="muted small">Forgot your password? The server admin can reset it with <code>cupful reset-password</code>.</p>
</div>
