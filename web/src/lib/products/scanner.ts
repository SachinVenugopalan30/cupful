// SPDX-License-Identifier: AGPL-3.0-or-later
// Reads a barcode from a playing <video>: the browser's BarcodeDetector where it reads retail codes,
// else the zxing-wasm reader, imported only here, when Scan is tapped.

import { NATIVE_FORMATS, normalizeScan, pickDecoder } from './logic';

export type Read = { rawValue: string; format: string };
export type Detect = (video: HTMLVideoElement) => Promise<Read | null>;

/**
 * A rear camera at a useful resolution, asking for continuous autofocus where the browser supports
 * it. Without this some Android cameras start focused at infinity, so only a barcode held far away
 * is sharp. Browsers ignore the constraint they do not know.
 */
export function cameraConstraints(): MediaStreamConstraints {
	return {
		audio: false,
		video: {
			facingMode: { ideal: 'environment' },
			width: { ideal: 1920 },
			height: { ideal: 1080 },
			advanced: [{ focusMode: 'continuous' } as MediaTrackConstraintSet]
		}
	};
}

type FocusCaps = MediaTrackCapabilities & {
	focusMode?: string[];
	focusDistance?: { min: number; max: number };
};
const focus = (c: Record<string, unknown>) => ({ advanced: [c as MediaTrackConstraintSet] });

/** 20 cm: where a hand-held product sits when you scan it, within what the lens can reach. */
export function nearFocusDistance({ min, max }: { min: number; max: number }): number {
	return Math.min(Math.max(0.2, min), max);
}

/**
 * Gets the lens off infinity. Some Android cameras report continuous autofocus yet start parked at
 * infinity, so only a far-away barcode is sharp. Moving the lens near first, then handing over to
 * continuous autofocus, restarts it from a useful place; where continuous isn't offered, a fixed
 * near focus is the next best thing. `none`: the camera exposes no focus control at all. Safe to
 * call again to refocus.
 */
export async function startFocus(
	track: MediaStreamTrack,
	wait: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms))
): Promise<'auto' | 'near' | 'none'> {
	const caps = track.getCapabilities?.() as FocusCaps | undefined;
	const modes = caps?.focusMode ?? [];
	const near = caps?.focusDistance && modes.includes('manual') ? nearFocusDistance(caps.focusDistance) : null;
	try {
		if (modes.includes('continuous')) {
			if (near !== null) {
				await track.applyConstraints(focus({ focusMode: 'manual', focusDistance: near }));
				await wait(500);
			}
			await track.applyConstraints(focus({ focusMode: 'continuous' }));
			return 'auto';
		}
		if (near !== null) {
			await track.applyConstraints(focus({ focusMode: 'manual', focusDistance: near }));
			return 'near';
		}
	} catch {
		// A camera that refuses these constraints keeps whatever focus it has.
	}
	return 'none';
}

/** Brave's camera often can't focus close up, even with shields off; Chrome can. */
export async function isBrave(nav: { brave?: { isBrave?: () => Promise<boolean> } }): Promise<boolean> {
	try {
		return (await nav.brave?.isBrave?.()) === true;
	} catch {
		return false;
	}
}

// Not in TypeScript's DOM lib yet.
type BarcodeDetectorCtor = {
	new (o: { formats: string[] }): { detect(v: HTMLVideoElement): Promise<Read[]> };
	getSupportedFormats(): Promise<string[]>;
};

export async function makeDetector(): Promise<Detect> {
	const Native = (globalThis as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector;
	if (Native && (await pickDecoder({ BarcodeDetector: Native })) === 'native') {
		const d = new Native({ formats: NATIVE_FORMATS });
		return async (v) => (await d.detect(v))[0] ?? null;
	}
	return (await import('./zxing')).detect;
}

/** Polls `detect` until it reads a valid GTIN (returned) or `signal` aborts (null). */
export async function scanLoop(
	video: HTMLVideoElement,
	detect: Detect,
	signal: AbortSignal
): Promise<string | null> {
	while (!signal.aborted) {
		await new Promise((r) => setTimeout(r, 150));
		if (signal.aborted || video.readyState < 2) continue;
		const r = await detect(video).catch(() => null);
		const code = r && normalizeScan(r.rawValue, r.format);
		if (code) return code;
	}
	return null;
}
