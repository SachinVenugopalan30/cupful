<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  AboutSection — running version and source link (AGPL network clause), license, Open Food Facts
  credit and privacy notes. No props.
-->
<script lang="ts">
	import { errorMessage, health, type Health } from '#lib/api.ts';

	let info = $state<Health | null>(null);
	let error = $state('');
	health().then(
		(h) => (info = h),
		(e) => (error = errorMessage(e))
	);
</script>

<section class="card stack" aria-labelledby="about-h">
	<h2 id="about-h">About</h2>
	{#if error}
		<p class="notice notice-error" role="alert">{error}</p>
	{:else}
		<p aria-busy={!info}>
			Cupful <span class="num">{info ? info.version : '…'}</span>
			{#if info}· <a href={info.source_url} rel="noopener">Source code</a>{/if}
		</p>
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
</section>

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
