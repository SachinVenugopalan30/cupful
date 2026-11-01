<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import ProvidersSection from '#lib/settings/ProvidersSection.svelte';
	import TokensSection from '#lib/settings/TokensSection.svelte';
	import GoogleHealth from '#lib/settings/GoogleHealth.svelte';
	import DataSection from '#lib/settings/DataSection.svelte';
	import SyncSection from '#lib/settings/SyncSection.svelte';
	import UsersSection from '#lib/settings/UsersSection.svelte';
	import AboutSection from '#lib/settings/AboutSection.svelte';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import { session } from '#lib/session.svelte.ts';
	import { loadTheme, saveTheme, type ThemePref } from '#lib/theme.ts';

	let theme = $state<ThemePref>(loadTheme());
	$effect(() => saveTheme(theme));
</script>

<svelte:head><title>Settings · Cupful</title></svelte:head>

<div class="settings stack">
	<h1>Settings</h1>
	<p class="muted small">Settings save to the server, so they need a connection.</p>

	<SettingRow
		headingId="profile-link-h"
		title="Profile and targets"
		description="Units, time zone, goal and pace."
	>
		<p><a class="btn" href="/profile">Open profile</a></p>
	</SettingRow>

	<SettingRow
		headingId="appearance-h"
		title="Appearance"
		description="Follow the system, or choose light or dark on this device."
	>
		<fieldset class="segmented">
			<legend class="visually-hidden">Theme</legend>
			{#each [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as [value, label] (value)}
				<label><input type="radio" name="theme" {value} bind:group={theme} />{label}</label>
			{/each}
		</fieldset>
		<p class="hint">Remembered on this device. System follows your phone or computer.</p>
	</SettingRow>

	<TokensSection />
	<GoogleHealth />
	<ProvidersSection isAdmin={session.user?.is_admin ?? false} />
	<DataSection />
	<SyncSection />
	<UsersSection isAdmin={session.user?.is_admin ?? false} />
	<AboutSection />

</div>

<style>
	.settings {
		max-width: var(--page-max);
		margin: 0 auto;
	}
</style>
