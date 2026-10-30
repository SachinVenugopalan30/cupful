<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import ProfileSection from '#lib/settings/ProfileSection.svelte';
	import ProvidersSection from '#lib/settings/ProvidersSection.svelte';
	import TokensSection from '#lib/settings/TokensSection.svelte';
	import GoogleHealth from '#lib/settings/GoogleHealth.svelte';
	import DataSection from '#lib/settings/DataSection.svelte';
	import SyncSection from '#lib/settings/SyncSection.svelte';
	import AboutSection from '#lib/settings/AboutSection.svelte';
	import { auth, errorMessage } from '#lib/api.ts';
	import { session } from '#lib/session.svelte.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { clearUser, countFor } from '#lib/offline/queue.ts';
	import { clearPreviousUser, clearSessionCache } from '#lib/offline/session-cache.ts';
	import { loadTheme, saveTheme, type ThemePref } from '#lib/theme.ts';

	let theme = $state<ThemePref>(loadTheme());
	$effect(() => saveTheme(theme));

	let logoutError = $state('');
	let confirmingLogout = $state(false);
	let waiting = $state(0);
	let loggingOut = $state(false);

	async function askLogout() {
		logoutError = '';
		if (!connection.online) {
			logoutError = 'Logging out needs a connection. Reconnect so queued changes can sync first.';
			return;
		}
		const userId = connection.activeUserId;
		if (userId !== null) {
			waiting = await countFor(userId);
			if (waiting > 0) {
				confirmingLogout = true;
				return;
			}
		}
		await logout();
	}

	async function logout() {
		loggingOut = true;
		logoutError = '';
		const userId = connection.activeUserId;
		try {
			// End the server session first: if this fails, nothing local has been deleted.
			await auth.logout();
			if (userId !== null) {
				try {
					await clearUser(userId);
					await clearSessionCache(userId);
					await clearPreviousUser();
				} catch {
					// The next login clears any leftovers; being logged out still matters more.
				}
			}
			// Full reload: no state from this account survives into the next one.
			location.assign('/login');
		} catch (e) {
			logoutError = errorMessage(e);
			confirmingLogout = false;
		} finally {
			loggingOut = false;
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
	<GoogleHealth />
	<ProvidersSection isAdmin={session.user?.is_admin ?? false} />
	<DataSection />
	<SyncSection />
	<AboutSection />

	<section class="card stack" aria-labelledby="account-h">
		<h2 id="account-h">Account</h2>
		<p>Logged in as <strong>{session.user?.username}</strong>.</p>
		{#if logoutError}<p class="notice notice-error" role="alert">{logoutError}</p>{/if}
		{#if confirmingLogout}
			<div class="notice notice-warning" role="group" aria-label="Confirm logout">
				<p>
					{waiting} {waiting === 1 ? 'change hasn’t' : 'changes haven’t'} synced. Logging out discards
					{waiting === 1 ? ' it' : ' them'} from this device.
				</p>
				<div class="row">
					<button class="btn btn-danger" onclick={logout} disabled={loggingOut}>
						Log out and discard
					</button>
					<button class="btn btn-quiet" onclick={() => (confirmingLogout = false)} disabled={loggingOut}>
						Cancel
					</button>
				</div>
			</div>
		{:else}
			<p>
				<button class="btn btn-danger" onclick={askLogout} disabled={loggingOut}>
					{loggingOut ? 'Logging out…' : 'Log out'}
				</button>
			</p>
		{/if}
	</section>
</div>

<style>
	.settings {
		max-width: var(--content-max);
		margin: 0 auto;
	}
</style>
