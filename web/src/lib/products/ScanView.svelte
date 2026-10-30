<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Camera scan plus the typed-barcode fallback; looks the code up. The camera stops on unmount. -->
<script lang="ts">
	import { ApiError, errorMessage, products, type Product } from '../api';
	import { cameraProblem, normalizeScan } from './logic';
	import { applyContinuousFocus, cameraConstraints, makeDetector, scanLoop } from './scanner';

	let {
		onFound,
		onLabel,
		onSaved
	}: { onFound: (p: Product) => void; onLabel: (barcode: string) => void; onSaved: () => void } =
		$props();

	let camera = $state(true);
	let camError = $state('');
	let status = $state('');
	let video = $state<HTMLVideoElement>();

	let typed = $state('');
	let typedError = $state('');
	let barcode = $state('');
	let busy = $state(false);
	let error = $state<{ text: string; code?: string } | null>(null);

	$effect(() => {
		if (!camera || !video) return;
		const v = video;
		const ac = new AbortController();
		let stream: MediaStream | undefined;
		const fail = (msg: string) => {
			if (ac.signal.aborted) return;
			camError = msg;
			status = '';
			camera = false;
		};
		(async () => {
			const pre = cameraProblem({
				isSecureContext: window.isSecureContext,
				hasMediaDevices: !!navigator.mediaDevices?.getUserMedia
			});
			if (pre) return fail(pre);
			status = 'Starting the camera…';
			let detect;
			try {
				detect = await makeDetector();
			} catch {
				return fail("The barcode reader couldn't load. Type the barcode below.");
			}
			try {
				stream = await navigator.mediaDevices.getUserMedia(cameraConstraints());
			} catch (e) {
				return fail(cameraProblem({ error: e }) ?? '');
			}
			if (ac.signal.aborted) return stream.getTracks().forEach((t) => t.stop());
			v.srcObject = stream;
			await v.play().catch(() => {});
			// Some Android cameras only start autofocusing once asked after the stream is live.
			const track = stream.getVideoTracks()[0];
			if (track) void applyContinuousFocus(track);
			status = 'Point the camera at the barcode.';
			const code = await scanLoop(v, detect, ac.signal);
			if (code) {
				camera = false; // re-runs this effect's cleanup: the camera stops
				lookup(code);
			}
		})();
		return () => {
			ac.abort();
			stream?.getTracks().forEach((t) => t.stop());
			v.srcObject = null;
		};
	});

	async function lookup(code: string) {
		barcode = code;
		busy = true;
		error = null;
		status = `Found ${code}. Looking it up…`;
		try {
			onFound(await products.lookup(code));
		} catch (e) {
			if (e instanceof ApiError && e.code === 'product_not_found') return onLabel(code);
			status = '';
			error = { text: errorMessage(e, 'product'), code: e instanceof ApiError ? e.code : undefined };
		} finally {
			busy = false;
		}
	}

	function submitTyped(e: SubmitEvent) {
		e.preventDefault();
		const code = normalizeScan(typed.replace(/\s/g, ''));
		typedError = code ? '' : 'Check the barcode: 8–14 digits, as printed under the bars.';
		if (code) {
			camera = false;
			lookup(code);
		}
	}
</script>

<div class="stack">
	{#if camera}
		<div class="viewfinder">
			<video
				bind:this={video}
				muted
				playsinline
				aria-label="Camera view. Point it at the product's barcode; it's read automatically."
			></video>
			<div class="aim" aria-hidden="true"></div>
		</div>
	{:else if camError}
		<p class="notice notice-warning" role="alert">{camError}</p>
	{:else if !busy}
		<button type="button" class="btn" onclick={() => ((camera = true), (error = null))}>Scan again</button>
	{/if}

	<p class="small muted" aria-live="polite">{status}</p>

	{#if error}
		<div class="notice notice-error" role="alert">
			<p>{error.text}</p>
			<div class="row">
				{#if error.code !== 'rate_limited'}
					<button type="button" class="btn" onclick={() => lookup(barcode)}>Try again</button>
				{/if}
				{#if error.code === 'lookup_failed' || error.code === 'rate_limited'}
					<button type="button" class="btn" onclick={() => onLabel(barcode)}>Enter from the label</button>
				{/if}
			</div>
		</div>
	{/if}

	<form class="field" onsubmit={submitTyped} novalidate>
		<label for="typed-barcode">Or type the barcode</label>
		<div class="typed">
			<input
				id="typed-barcode"
				inputmode="numeric"
				autocomplete="off"
				enterkeyhint="search"
				bind:value={typed}
				aria-invalid={typedError ? 'true' : undefined}
				aria-describedby={typedError ? 'typed-error' : undefined}
			/>
			<button class="btn btn-primary" disabled={busy}>Look up</button>
		</div>
		{#if typedError}<p id="typed-error" class="small error">{typedError}</p>{/if}
	</form>

	<button type="button" class="btn btn-quiet" onclick={onSaved}>Saved products</button>
</div>

<style>
	.viewfinder {
		position: relative;
		aspect-ratio: 4 / 3;
		border-radius: var(--radius-md);
		overflow: hidden;
		background: #000;
	}
	video {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.aim {
		position: absolute;
		inset: 30% 10%;
		border: 2px solid rgb(255 255 255 / 0.85);
		border-radius: var(--radius-sm);
		box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.3);
	}
	.typed {
		display: flex;
		gap: var(--space-2);
	}
	.typed .btn {
		white-space: nowrap;
	}
	.error {
		color: var(--error);
	}
</style>
