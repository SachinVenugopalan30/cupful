<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import '#lib/styles/tokens.css';
	import '#lib/styles/global.css';
	import favicon from '#lib/assets/favicon.svg';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { loadSession, redirectFor, session } from '#lib/session.svelte.ts';
	import { initConnection } from '#lib/offline/state.svelte.ts';
	import { initUpdatePrompt } from '#lib/offline/update.svelte.ts';
	import SyncBar from '#lib/offline/SyncBar.svelte';
	import UpdateBanner from '#lib/offline/UpdateBanner.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	onMount(() => {
		initConnection();
		initUpdatePrompt();
	});

	let loadError = $state(false);
	async function load() {
		loadError = false;
		try {
			await loadSession();
		} catch {
			loadError = true;
		}
	}
	load();

	// First run → /setup, logged out → /login, logged in → away from those two.
	const target = $derived(session.auth ? redirectFor(page.url.pathname, session.auth) : null);
	$effect(() => {
		if (target) goto(target, { replaceState: true });
	});
	const ready = $derived(session.auth !== null && target === null);
	const path = $derived(page.url.pathname);
	/** `aria-current` for a nav link pointing at `href`. */
	const current = (href: string) => (path === href ? 'page' : undefined);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Cupful</title>
</svelte:head>

{#if loadError}
	<main class="page">
		<div class="notice notice-error" role="alert">
			<p>Can't reach Cupful. Check your connection, then try again.</p>
			<p><button class="btn" onclick={load}>Try again</button></p>
		</div>
	</main>
{:else if ready}
	{#if session.user}
		<nav class="tabs" aria-label="Main">
			<span class="brand" aria-hidden="true">Cupful</span>
			<a href="/" aria-current={current('/')}>
				<svg viewBox="0 0 24 24" aria-hidden="true"
					><path d="M4 7h13v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5zM17 9h1.5a2.5 2.5 0 0 1 0 5H17" /></svg
				>
				<span>Today</span>
			</a>
			<!-- On wide screens Today shows the Activity section itself. -->
			<a href="/activity" class="activity-tab" aria-current={current('/activity')}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12h4l3-7 4 14 3-7h4" /></svg>
				<span>Activity</span>
			</a>
			<a href="/trends" aria-current={current('/trends')}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20V10M10 20V4M16 20v-7M22 20H2" /></svg>
				<span>Trends</span>
			</a>
			<a href="/settings" aria-current={current('/settings')}>
				<svg viewBox="0 0 24 24" aria-hidden="true"
					><path d="M4 7h9M17 7h3M4 17h3M11 17h9" /><circle cx="15" cy="7" r="2" /><circle
						cx="9"
						cy="17"
						r="2"
					/></svg
				>
				<span>Settings</span>
			</a>
		</nav>
	{/if}
	<main class="page" class:with-tabs={!!session.user}>
		<UpdateBanner />
		{#if session.user}<SyncBar />{/if}
		{@render children()}
	</main>
{:else}
	<main class="page" aria-busy="true"><span class="visually-hidden">Loading</span></main>
{/if}

<style>
	/* Pages narrower than this (forms, settings) set max-width: var(--content-max) themselves. */
	.page {
		max-width: 64rem;
		margin: 0 auto;
		padding: var(--space-4) var(--gutter) var(--space-6);
		padding-left: max(var(--gutter), env(safe-area-inset-left));
		padding-right: max(var(--gutter), env(safe-area-inset-right));
	}
	/* Room for the bottom tab bar on phones. */
	.with-tabs {
		padding-bottom: calc(var(--touch) + var(--space-8) + env(safe-area-inset-bottom));
	}

	.tabs {
		position: fixed;
		inset: auto 0 0;
		z-index: 10;
		display: flex;
		justify-content: space-around;
		padding: var(--space-1) var(--gutter) env(safe-area-inset-bottom);
		background: var(--surface);
		border-top: 1px solid var(--border);
	}
	.brand {
		display: none;
	}
	.tabs a {
		flex: 1;
		max-width: 8rem;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.125rem;
		min-height: 3.25rem;
		justify-content: center;
		border-radius: var(--radius-md);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-decoration: none;
	}
	.tabs a[aria-current='page'] {
		color: var(--accent);
	}
	.tabs svg {
		width: 1.5rem;
		height: 1.5rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.75;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	/* Wide screens: a top bar with the wordmark. */
	@media (min-width: 48rem) {
		.tabs {
			position: sticky;
			inset: 0 0 auto;
			justify-content: flex-start;
			gap: var(--space-2);
			padding: var(--space-2) max(var(--gutter), calc((100vw - 64rem) / 2));
			border-top: 0;
			border-bottom: 1px solid var(--border);
		}
		.brand {
			display: block;
			margin-right: auto;
			align-self: center;
			font-family: var(--font-numeric);
			font-size: var(--text-lg);
			font-weight: var(--weight-bold);
			color: var(--accent);
		}
		.tabs a {
			flex: none;
			flex-direction: row;
			gap: var(--space-2);
			min-height: var(--touch);
			padding: 0 var(--space-3);
			font-size: var(--text-sm);
		}
		.tabs a[aria-current='page'] {
			background: var(--accent-soft);
		}
		.tabs svg {
			width: 1.25rem;
			height: 1.25rem;
		}
		.with-tabs {
			padding-bottom: var(--space-8);
		}
		.page {
			padding-top: var(--space-5);
		}
	}
	@media (min-width: 60rem) {
		.tabs .activity-tab {
			display: none;
		}
	}
</style>
