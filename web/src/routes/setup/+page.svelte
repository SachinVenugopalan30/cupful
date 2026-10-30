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
	<p class="wordmark" aria-hidden="true">Cupful</p>
	<h1>Create the admin account</h1>
	<p class="muted">This is the first account on this server. You can add others later.</p>
	<form class="card stack" onsubmit={submit}>
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
		<p class="muted small">
			Cupful <span class="num">{server.version}</span> ·
			<a href={server.source_url} rel="noopener">Source code</a> (AGPL-3.0-or-later)
		</p>
	{:else}
		<p class="muted small">Free software under AGPL-3.0-or-later.</p>
	{/if}
</div>
