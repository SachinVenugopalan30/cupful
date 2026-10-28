<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ProvidersSection — AI providers: list, add, edit, delete, defaults for text and photos,
  Test connection. Says that photos are sent to the configured provider.
  Props (fixed contract; Settings page passes this):
    isAdmin: boolean   provider routes are admin-only (403 otherwise); non-admins see a
                       read-only note instead of the editor
  API: providers.list/create/update/remove/test from #lib/api. Keys are write-only (`has_key`).
-->
<script lang="ts">
	let { isAdmin }: { isAdmin: boolean } = $props();
</script>

<section class="stub" aria-labelledby="providers-h">
	<h2 id="providers-h">AI providers</h2>
	<p>ProvidersSection placeholder · admin: {isAdmin}</p>
</section>
