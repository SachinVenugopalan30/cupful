<!--
  CountUp — a number that counts to its value over --dur-slow (520 ms): from 0 on mount, and from the
  number on screen to the new one whenever `value` changes. Reduced motion jumps straight to the
  value. An invisible copy of the final text reserves its width so the count never shifts the layout.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { startTween, tweenValue, type Tween } from './count-up.ts';

	let {
		value,
		decimals = 0,
		prefix = ''
	}: { value: number; decimals?: number; prefix?: string } = $props();

	const format = (n: number) =>
		n.toLocaleString('en-US', {
			minimumFractionDigits: decimals,
			maximumFractionDigits: decimals
		});

	let shown = $state(0);
	const finalText = $derived(prefix + format(value));

	/** --dur-slow in ms, read from the token so the scale stays in one place; 0 under reduced motion. */
	function duration(): number {
		const raw = getComputedStyle(document.documentElement).getPropertyValue('--dur-slow').trim();
		const ms = parseFloat(raw);
		return Number.isFinite(ms) ? (raw.endsWith('ms') ? ms : ms * 1000) : 520;
	}
	const reduced = () =>
		typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

	let frame = 0;
	function countTo(target: number) {
		cancelAnimationFrame(frame);
		const tween: Tween = startTween(shown, target, performance.now(), duration(), reduced());
		const tick = (now: number) => {
			shown = tweenValue(tween, now);
			if (now - tween.start < tween.duration) frame = requestAnimationFrame(tick);
			else shown = target;
		};
		frame = requestAnimationFrame(tick);
	}

	// Runs on mount (from 0) and on every change of `value` (from what is on screen).
	$effect(() => {
		const target = value;
		untrack(() => countTo(target));
		return () => cancelAnimationFrame(frame);
	});
</script>

<span class="num count" data-final={finalText}><span>{prefix}{format(shown)}</span></span>

<style>
	/*
	 * The final text, drawn invisibly in the same grid cell, reserves exactly its width in whatever
	 * font the figure uses, so counting never shifts the layout. A pseudo-element keeps it out of the
	 * text content that assistive tech and tests read.
	 */
	.count {
		display: inline-grid;
		justify-items: start;
	}
	.count > span,
	.count::after {
		grid-area: 1 / 1;
	}
	.count::after {
		content: attr(data-final);
		visibility: hidden;
	}
</style>
