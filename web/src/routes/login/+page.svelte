<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError, auth, errorMessage } from '#lib/api.ts';
	import { loadSession } from '#lib/session.svelte.ts';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await auth.login({ username, password });
			await loadSession();
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
		{#if error}
			<p class="notice notice-error" role="alert">{error}</p>
		{/if}
		<button class="btn btn-primary btn-block" disabled={busy}>{busy ? 'Logging in…' : 'Log in'}</button>
	</form>
	<p class="muted small">Forgot your password? The server admin can reset it with <code>cupful reset-password</code>.</p>
</div>

