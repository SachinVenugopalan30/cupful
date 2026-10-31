<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Invite acceptance: a public screen. The one-time token arrives in `?token=`, is read once, then
  replaced out of the address bar before the form appears. Success logs the new account in.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { health, invites, type Health } from '#lib/api.ts';
	import { inviteErrorMessage, inviteToken } from '#lib/invites.ts';
	import { loadSession } from '#lib/session.svelte.ts';

	const zones = Intl.supportedValuesOf('timeZone');

	// Read the token once and strip it from the address bar, so it stays out of history and copied
	// URLs. A reload after this point needs the original link again.
	let token = $state('');
	if (typeof window !== 'undefined') {
		token = inviteToken(window.location.search);
		if (window.location.search) window.history.replaceState(window.history.state, '', '/invite');
	}

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
			await invites.accept({ token, username, password, timezone });
			await loadSession();
			await goto('/', { replaceState: true });
		} catch (err) {
			error = inviteErrorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>Join · Cupful</title></svelte:head>

<div class="auth-page">
	<div class="auth-card">
		<p class="wordmark" aria-hidden="true">Cupful</p>
		<h1>Join Cupful</h1>
		{#if !token}
			<div class="notice notice-error" role="alert">
				<p>This invite link is missing its token. Ask the admin for a new link.</p>
			</div>
			<p class="muted small auth-meta"><a href="/login">Go to log in</a></p>
		{:else}
			<p class="muted auth-copy">Choose a username and password for your account.</p>
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
			<p class="muted small auth-meta">Already have an account? <a href="/login">Log in</a>.</p>
		{/if}
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
	.wordmark,
	.auth-card h1,
	.auth-copy {
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
