<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import LogoutButton from '#lib/LogoutButton.svelte';
	import ProfileSection from '#lib/settings/ProfileSection.svelte';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import { session } from '#lib/session.svelte.ts';
</script>

<svelte:head><title>Profile · Cupful</title></svelte:head>

<div class="profile stack">
	<header class="page-head">
		<h1>Profile</h1>
		<a class="btn btn-quiet" href="/settings">Settings</a>
	</header>
	<p class="muted small">Targets, units and time zone. Saving creates a new profile version from today.</p>

	<ProfileSection />

	<SettingRow headingId="account-h" title="Account" description="Log out of this device.">
		<p>Logged in as <strong>{session.user?.username}</strong>.</p>
		<LogoutButton />
	</SettingRow>
</div>

<style>
	.profile {
		max-width: var(--page-max);
		margin: 0 auto;
	}
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
</style>
