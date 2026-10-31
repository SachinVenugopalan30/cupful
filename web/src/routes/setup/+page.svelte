<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { ApiError, auth, errorMessage, health, type Health } from '#lib/api.ts';
	import { loadSession } from '#lib/session.svelte.ts';

	const zones = Intl.supportedValuesOf('timeZone');
	let username = $state('');
	let password = $state('');
	let timezone = $state(Intl.DateTimeFormat().resolvedOptions().timeZone);
	let error = $state('');
	let busy = $state(false);
	/** Running version and AGPL source offer. Absent when the server cannot be reached. */
	let server = $state<Health | null>(null);

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

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await auth.setup({ username, password, timezone });
			await loadSession();
			await goto('/', { replaceState: true });
		} catch (err) {
			// 409: someone else finished setup first; the layout sends us to login on reload.
			error =
				err instanceof ApiError && err.status === 409
					? 'Setup is already done. Log in instead.'
					: errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>Set up · Cupful</title></svelte:head>

<div class="auth-page">
	<div class="auth-card">
		<p class="wordmark" aria-hidden="true">Cupful</p>
		<div class="auth-intro">
			<h1>Create the admin account</h1>
			<p class="muted">This is the first account on this server. You can add others later.</p>
		</div>
		<form class="stack" onsubmit={submit}>
			<div class="field">
				<label for="username">Username</label>
				<input
					id="username"
					bind:value={username}
					autocomplete="username"
					autocapitalize="none"
					maxlength="32"
					aria-describedby="username-hint"
					required
				/>
				<p id="username-hint" class="hint">Letters, digits, “_”, “.” or “-”.</p>
			</div>
			<div class="field">
				<label for="password">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					autocomplete="new-password"
					minlength="8"
					aria-describedby="password-hint"
					required
				/>
				<p id="password-hint" class="hint">At least 8 characters.</p>
			</div>
			<div class="field">
				<label for="timezone">Time zone</label>
				<select id="timezone" bind:value={timezone} aria-describedby="timezone-hint">
					{#if !zones.includes(timezone)}<option>{timezone}</option>{/if}
					{#each zones as z (z)}<option>{z}</option>{/each}
				</select>
				<p id="timezone-hint" class="hint">Your days start at midnight here.</p>
			</div>
			{#if error}
				<p class="notice notice-error" role="alert">{error}</p>
			{/if}
			<button class="btn btn-primary btn-block" disabled={busy}>
				{busy ? 'Creating…' : 'Create account'}
			</button>
		</form>
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
	.wordmark {
		margin: 0;
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
	input,
	select {
		transition:
			border-color var(--dur-fast) var(--ease),
			background var(--dur-fast) var(--ease);
	}
</style>
