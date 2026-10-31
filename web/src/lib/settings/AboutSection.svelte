<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  AboutSection — app build, running server version and source link (AGPL network clause), license,
  Open Food Facts credit and privacy notes. Online it asks /api/health; offline it shows the cached
  server version if there is one, and always the build version. No props.
-->
<script lang="ts">
	import { version } from '$app/env';
	import { errorMessage, health, type Health } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { loadHealth } from '#lib/offline/session-cache.ts';
	import SettingRow from './SettingRow.svelte';

	let server = $state<Health | null>(null);
	let cached = $state<Health | null>(null);
	let error = $state('');

	async function load() {
		error = '';
		server = null;
		if (connection.online) {
			try {
				server = await health();
			} catch (e) {
				error = errorMessage(e);
			}
		}
		cached = server ? null : await loadHealth().catch(() => null);
	}

	$effect(() => {
		void connection.online;
		void load();
	});
</script>

<SettingRow headingId="about-h" title="About" description="Free software under AGPL-3.0-or-later.">
	<p>App build <span class="num">{version}</span></p>
	{#if server}
		<p>
			Cupful <span class="num">{server.version}</span>
			· <a href={server.source_url} rel="noopener">Source code</a>
		</p>
	{:else if cached}
		<p>
			Cupful <span class="num">{cached.version}</span> (cached)
			· <a href={cached.source_url} rel="noopener">Source code</a>
		</p>
	{:else if !connection.online}
		<p class="small muted">The server version isn't available offline yet.</p>
	{/if}
	{#if error}
		<p class="notice notice-error" role="alert">{error}</p>
	{/if}
	<p class="small">
		Free software under the GNU Affero General Public License, version 3 or later
		(AGPL-3.0-or-later). If you run a modified version for others, share its source.
	</p>
	<ul class="small notes">
		<li>
			Product data from <a href="https://world.openfoodfacts.org" rel="noopener">Open Food Facts</a>
			(<a href="https://opendatacommons.org/licenses/odbl/1-0/" rel="noopener">ODbL</a>). Each barcode
			lookup tells Open Food Facts which barcode was scanned; admins can turn lookups off with
			<code>CUPFUL_PRODUCT_LOOKUP=off</code>.
		</li>
		<li>
			Photos are used for an estimate only and aren't saved; they're sent to your configured AI
			provider.
		</li>
		<li>No telemetry. Cupful doesn't report anything to anyone.</li>
	</ul>
</SettingRow>

<style>
	.notes {
		margin: 0;
		padding-left: var(--space-5);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	code {
		overflow-wrap: anywhere;
	}
</style>
