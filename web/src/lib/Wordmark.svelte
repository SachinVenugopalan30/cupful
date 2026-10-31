<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Wordmark — the shipped Cupful logo (bowl plus outlined Manrope). The package ships one light and
  one dark SVG; CSS picks the right one from `prefers-color-scheme` or `<html data-theme>`.
  Props:
    label?: string   accessible name; omit when the surrounding heading already says "Cupful"
    height?: string  CSS height for the logo, default 1.5rem
-->
<script lang="ts">
	import light from '#lib/assets/wordmark-light.svg';
	import dark from '#lib/assets/wordmark-dark.svg';

	let { label, height = '1.5rem' }: { label?: string; height?: string } = $props();
</script>

<span
	class="wordmark"
	style="--logo-height:{height}"
	aria-hidden={label ? undefined : 'true'}
>
	{#if label}<span class="visually-hidden">{label}</span>{/if}
	<img class="light" src={light} alt="" />
	<img class="dark" src={dark} alt="" />
</span>

<style>
	.wordmark {
		display: inline-flex;
		align-items: center;
		height: var(--logo-height);
		line-height: 0;
	}
	.wordmark img {
		display: block;
		width: auto;
		height: var(--logo-height);
	}
	img.dark {
		display: none;
	}
	@media (prefers-color-scheme: dark) {
		img.light {
			display: none;
		}
		img.dark {
			display: block;
		}
	}
	/* An explicit theme wins over the media query; app.html sets `data-theme` before first paint. */
	:global(html[data-theme='light']) img.light {
		display: block;
	}
	:global(html[data-theme='light']) img.dark {
		display: none;
	}
	:global(html[data-theme='dark']) img.light {
		display: none;
	}
	:global(html[data-theme='dark']) img.dark {
		display: block;
	}
</style>
