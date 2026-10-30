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

/** Turns on continuous autofocus after the stream starts, where the camera exposes the mode. */
export async function applyContinuousFocus(track: MediaStreamTrack): Promise<boolean> {
	const caps = track.getCapabilities?.() as (MediaTrackCapabilities & { focusMode?: string[] }) | undefined;
	if (!caps?.focusMode?.includes('continuous')) return false;
	try {
		await track.applyConstraints({
			advanced: [{ focusMode: 'continuous' } as MediaTrackConstraintSet]
		});
		return true;
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
