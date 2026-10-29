<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import ProfileSection from '#lib/settings/ProfileSection.svelte';
	import ProvidersSection from '#lib/settings/ProvidersSection.svelte';
	import TokensSection from '#lib/settings/TokensSection.svelte';
	import AboutSection from '#lib/settings/AboutSection.svelte';
	import { auth, errorMessage } from '#lib/api.ts';
	import { session } from '#lib/session.svelte.ts';
	import { loadTheme, saveTheme, type ThemePref } from '#lib/theme.ts';

	let theme = $state<ThemePref>(loadTheme());
	$effect(() => saveTheme(theme));

	let logoutError = $state('');
	async function logout() {
		logoutError = '';
		try {
			await auth.logout();
			// Full reload: no state from this account survives into the next one.
			location.assign('/login');
		} catch (e) {
			logoutError = errorMessage(e);
		}
	}
</script>

<svelte:head><title>Settings · Cupful</title></svelte:head>

<div class="settings stack">
	<h1>Settings</h1>
	<p class="muted small">Settings save to the server, so they need a connection.</p>

	<ProfileSection />

	<section class="card stack" aria-labelledby="appearance-h">
		<h2 id="appearance-h">Appearance</h2>
		<fieldset class="segmented">
			<legend class="visually-hidden">Theme</legend>
			{#each [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as [value, label] (value)}
				<label><input type="radio" name="theme" {value} bind:group={theme} />{label}</label>
			{/each}
		</fieldset>
		<p class="hint">Remembered on this device. System follows your phone or computer.</p>
	</section>

	<TokensSection />
	<ProvidersSection isAdmin={session.user?.is_admin ?? false} />
	<AboutSection />

	<section class="card stack" aria-labelledby="account-h">
		<h2 id="account-h">Account</h2>
		<p>Logged in as <strong>{session.user?.username}</strong>.</p>
		{#if logoutError}<p class="notice notice-error" role="alert">{logoutError}</p>{/if}
		<p><button class="btn btn-danger" onclick={logout}>Log out</button></p>
	</section>
</div>

<style>
	.settings {
		max-width: var(--content-max);
		margin: 0 auto;
	}
</style>
