<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ProfileSection — profile and targets (saving creates a new profile version from today), units,
  timezone. No props.
  API: profile.get/save from #lib/api. After saving, call loadSession() from #lib/session.svelte.ts
  so the timezone and units update app-wide. Units: format.ts (weightToKg, heightToCm).
-->
<section class="stub" aria-labelledby="profile-h">
	<h2 id="profile-h">Profile</h2>
	<p>ProfileSection placeholder</p>
</section>
