// SPDX-License-Identifier: AGPL-3.0-or-later
// Reads a barcode from a playing <video>: the browser's BarcodeDetector where it reads retail codes,
// else the zxing-wasm reader, imported only here, when Scan is tapped.

import { NATIVE_FORMATS, normalizeScan, pickDecoder } from './logic';

export type Read = { rawValue: string; format: string };
export type Detect = (video: HTMLVideoElement) => Promise<Read | null>;

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
