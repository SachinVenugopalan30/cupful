// SPDX-License-Identifier: AGPL-3.0-or-later
// The WASM barcode reader for browsers without BarcodeDetector (iOS Safari). scanner.ts imports this
// dynamically, so neither it nor the .wasm is in the main bundle. Cupful serves the .wasm itself;
// zxing-wasm would otherwise fetch it from a CDN.

import { prepareZXingModule, readBarcodes } from 'zxing-wasm/reader';
import wasmUrl from 'zxing-wasm/reader/zxing_reader.wasm?url';
import type { Read } from './scanner';

prepareZXingModule({
	overrides: {
		locateFile: (path: string, prefix: string) => (path.endsWith('.wasm') ? wasmUrl : prefix + path)
	}
});

let ctx: CanvasRenderingContext2D | null = null;

export async function detect(video: HTMLVideoElement): Promise<Read | null> {
	const { videoWidth: w, videoHeight: h } = video;
	if (!w || !h) return null;
	ctx ??= document.createElement('canvas').getContext('2d', { willReadFrequently: true });
	if (!ctx) return null;
	ctx.canvas.width = w;
	ctx.canvas.height = h;
	ctx.drawImage(video, 0, 0, w, h);
	const [r] = await readBarcodes(ctx.getImageData(0, 0, w, h), {
		formats: ['EAN13', 'EAN8', 'UPCA', 'UPCE'],
		maxNumberOfSymbols: 1,
		tryHarder: true
	});
	return r?.isValid ? { rawValue: r.text, format: r.format } : null;
}
