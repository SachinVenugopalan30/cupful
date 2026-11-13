<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import ProvidersSection from '#lib/settings/ProvidersSection.svelte';
	import TokensSection from '#lib/settings/TokensSection.svelte';
	import GoogleHealth from '#lib/settings/GoogleHealth.svelte';
	import DataSection from '#lib/settings/DataSection.svelte';
	import SyncSection from '#lib/settings/SyncSection.svelte';
	import UsersSection from '#lib/settings/UsersSection.svelte';
	import AboutSection from '#lib/settings/AboutSection.svelte';
	import { session } from '#lib/session.svelte.ts';
	import { loadTheme, saveTheme, type ThemePref } from '#lib/theme.ts';

	let theme = $state<ThemePref>(loadTheme());
	$effect(() => saveTheme(theme));

	let root: HTMLElement;
	const GROUPS = [
		['you', 'You'],
		['connections', 'Connections'],
		['your-data', 'Your data'],
		['server', 'Server']
	] as const;

	/** Opens the row the URL hash names (a link to /settings#sync, say). */
	function openFromHash() {
		const id = decodeURIComponent(location.hash.slice(1));
		const target = id ? document.getElementById(id) : null;
		const row = target?.closest('details') ?? target?.querySelector<HTMLDetailsElement>(':scope > details');
		if (row && root.contains(row)) row.open = true;
	}

	onMount(() => {
		// Wide screens show every section: open them all, and again if the window grows into it.
		const wide = matchMedia('(min-width: 60rem)');
		const openAll = () => {
			if (wide.matches) root.querySelectorAll('details').forEach((d) => (d.open = true));
		};
		openAll();
		openFromHash();
		wide.addEventListener('change', openAll);
		addEventListener('hashchange', openFromHash);
		return () => {
			wide.removeEventListener('change', openAll);
			removeEventListener('hashchange', openFromHash);
		};
	});
	// A same-page link to another hash is a client navigation; hashchange may not fire for it.
	afterNavigate(() => root && openFromHash());
</script>

<svelte:head><title>Settings · Cupful</title></svelte:head>

<div class="settings">
	<header>
		<h1>Settings</h1>
		<p class="muted small">Settings save to the server, so they need a connection.</p>
	</header>

	<div class="layout">
		<nav class="snav" aria-label="Settings sections">
			{#each GROUPS as [id, label] (id)}<a href="#group-{id}">{label}</a>{/each}
		</nav>

		<div class="groups" bind:this={root}>
			<div class="group" id="group-you" role="group" aria-labelledby="group-you-h">
				<p class="group-h" id="group-you-h">You</p>
				<div class="rows">
					<div class="inline-row">
						<div class="copy">
							<p class="t">Profile and targets</p>
							<p class="d">Units, time zone, goal and pace.</p>
						</div>
						<a class="btn" href="/profile">Open profile</a>
					</div>
					<div class="inline-row">
						<div class="copy">
							<p class="t" id="theme-t">Theme</p>
							<p class="d">Follows your device unless you choose. Remembered here.</p>
						</div>
						<fieldset class="segmented">
							<legend class="visually-hidden">Theme</legend>
							{#each [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as [value, label] (value)}
								<label><input type="radio" name="theme" {value} bind:group={theme} />{label}</label>
							{/each}
						</fieldset>
					</div>
				</div>
			</div>

			<div class="group" id="group-connections" role="group" aria-labelledby="group-connections-h">
				<p class="group-h" id="group-connections-h">Connections</p>
				<div class="rows">
					<TokensSection />
					<GoogleHealth />
					<ProvidersSection isAdmin={session.user?.is_admin ?? false} />
				</div>
			</div>

			<div class="group" id="group-your-data" role="group" aria-labelledby="group-your-data-h">
				<p class="group-h" id="group-your-data-h">Your data</p>
				<div class="rows">
					<DataSection />
					<SyncSection />
				</div>
			</div>

			<div class="group" id="group-server" role="group" aria-labelledby="group-server-h">
				<p class="group-h" id="group-server-h">Server</p>
				<div class="rows">
					<UsersSection isAdmin={session.user?.is_admin ?? false} />
					<AboutSection />
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.settings {
		display: grid;
		gap: var(--space-4);
	}
	header h1,
	header p {
		margin: 0;
	}
	.groups {
		display: grid;
		gap: var(--space-5);
		min-width: 0;
	}
	.group {
		display: grid;
		gap: var(--space-2);
		/* Anchor jumps land below the sticky top bar. */
		scroll-margin-top: calc(4rem + var(--space-3));
	}
	.group-h {
		margin: 0;
		padding-inline: var(--space-1);
		font-size: var(--text-sm);
		font-weight: var(--weight-bold);
		color: var(--text-muted);
	}
	/* Phone: one framed list per group, rows divided by hairlines. */
	.rows {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: clip;
	}
	.rows > :global(* + *) {
		border-top: 1px solid var(--border);
	}
	.inline-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2) var(--space-3);
		min-height: 3.5rem;
		padding: var(--space-2) var(--space-4);
	}
	.inline-row .copy {
		flex: 1 1 13rem;
		min-width: 0;
	}
	.inline-row .t,
	.inline-row .d {
		margin: 0;
	}
	.inline-row .t {
		font-weight: var(--weight-bold);
	}
	.inline-row .d {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}
	.inline-row .segmented {
		flex: 0 1 15rem;
	}
	.snav {
		display: none;
	}

	/* Desktop: a sticky list of anchor links beside every section, all open. */
	@media (min-width: 60rem) {
		.layout {
			display: grid;
			grid-template-columns: 14rem minmax(0, 1fr);
			gap: var(--space-6);
			align-items: start;
		}
		.snav {
			display: grid;
			gap: 2px;
			position: sticky;
			top: calc(4rem + var(--space-4));
		}
		.snav a {
			display: flex;
			align-items: center;
			min-height: var(--touch);
			padding: 0 var(--space-3);
			border-radius: var(--radius-sm);
			color: var(--text-muted);
			font-weight: var(--weight-bold);
			text-decoration: none;
			transition:
				background var(--dur-fast) var(--ease),
				color var(--dur-fast) var(--ease);
		}
		.snav a:hover {
			background: var(--surface-2);
			color: var(--text);
		}
		.rows {
			display: grid;
			gap: var(--space-3);
			background: none;
			border: 0;
			border-radius: 0;
			overflow: visible;
		}
		.rows > :global(* + *) {
			border-top: 0;
		}
		.inline-row {
			background: var(--surface);
			border: 1px solid var(--border);
			border-radius: var(--radius-lg);
			padding: var(--space-4) var(--space-5);
		}
	}
</style>
