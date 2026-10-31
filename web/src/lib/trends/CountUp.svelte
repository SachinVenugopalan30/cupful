<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  CountUp — a headline number that counts from zero to its value over 700 ms on mount.
  Trends remounts its content when the range changes, so the count runs again there.
  Reduced motion jumps straight to the value. The width is reserved for the final text so
  the count never shifts the layout.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { countValue } from './count-up.ts';

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
	const width = $derived(`${finalText.length}ch`);

	onMount(() => {
		if (typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches) {
			shown = value;
			return;
		}
		let frame = 0;
		const start = performance.now();
		const tick = (now: number) => {
			const progress = Math.min(1, (now - start) / 700);
			shown = countValue(0, value, progress);
			if (progress < 1) frame = requestAnimationFrame(tick);
		};
		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	});
</script>

<span class="num count" style:min-width={width}>{prefix}{format(shown)}</span>

<style>
	.count {
		display: inline-block;
		text-align: left;
	}
</style>
