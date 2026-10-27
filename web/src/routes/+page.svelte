<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	type Health = { status: string; version: string; source_url: string };
	let health = $state<Health | null>(null);
	fetch('/api/health')
		.then((r) => r.json())
		.then((h: Health) => (health = h))
		.catch(() => {});
</script>

<svelte:head><title>Cupful</title></svelte:head>

<main>
	<h1>Cupful</h1>
	<p>Nothing to log yet. The app is under construction.</p>
	{#if health}
		<p><small>v{health.version} · <a href={health.source_url}>Source code</a></small></p>
	{/if}
</main>
