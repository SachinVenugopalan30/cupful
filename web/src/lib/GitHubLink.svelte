<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  GitHubLink — the repo link and live star count in the nav. The server caches the count for an
  hour; a failed lookup leaves the link and hides only the number.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { compactStars, getGitHubInfo, GITHUB_URL } from './github.ts';

	let info = $state({ url: GITHUB_URL, stars: null as number | null });
	onMount(() => {
		void getGitHubInfo().then((next) => (info = next));
	});

	const count = $derived(info.stars === null ? null : compactStars(info.stars));
	const label = $derived(
		count === null
			? 'Cupful on GitHub'
			: `Cupful on GitHub, ${count} ${info.stars === 1 ? 'star' : 'stars'}`
	);
</script>

<a class="github" href={info.url} target="_blank" rel="noopener noreferrer" aria-label={label} title={label}>
	<svg class="mark" viewBox="0 0 24 24" aria-hidden="true">
		<path
			fill="currentColor"
			d="M12 .5C5.73.5.5 5.73.5 12c0 5.08 3.29 9.39 7.86 10.91.58.11.79-.25.79-.56 0-.28-.01-1.02-.02-2-3.2.7-3.88-1.54-3.88-1.54-.52-1.33-1.28-1.68-1.28-1.68-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.23-1.28-5.23-5.7 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11.1 11.1 0 0 1 5.8 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.84 1.19 3.1 0 4.43-2.69 5.41-5.25 5.69.41.35.78 1.05.78 2.12 0 1.53-.01 2.76-.01 3.14 0 .31.21.67.8.56A11.5 11.5 0 0 0 23.5 12C23.5 5.73 18.27.5 12 .5z"
		/>
	</svg>
	{#if count}
		<span class="stars">
			<svg viewBox="0 0 24 24" aria-hidden="true">
				<path
					fill="none"
					stroke="currentColor"
					stroke-width="1.6"
					stroke-linejoin="round"
					d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9z"
				/>
			</svg>
			{count}
		</span>
	{/if}
</a>

<style>
	.github {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.25rem;
		min-width: 2.75rem;
		min-height: var(--touch);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		color: var(--text-muted);
		font-size: var(--text-xs);
		text-decoration: none;
		transition:
			background-color var(--dur-fast) var(--ease),
			color var(--dur-fast) var(--ease);
	}
	.github:hover {
		background: var(--surface-2);
		color: var(--text);
	}
	.github:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.mark {
		width: 1.25rem;
		height: 1.25rem;
	}
	.stars {
		display: inline-flex;
		align-items: center;
		gap: 0.125rem;
		font-variant-numeric: tabular-nums;
	}
	.stars svg {
		width: 0.875rem;
		height: 0.875rem;
	}
</style>
