// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it, vi } from 'vitest';
import { applyContinuousFocus, cameraConstraints } from './scanner.ts';

function track(caps: unknown, apply = vi.fn(async () => {})) {
	return { getCapabilities: () => caps, applyConstraints: apply } as unknown as MediaStreamTrack;
}

describe('camera setup', () => {
	it('asks for the rear camera at 1080p with continuous focus', () => {
		const video = cameraConstraints().video as MediaTrackConstraints;
		expect(video.facingMode).toEqual({ ideal: 'environment' });
		expect(video.width).toEqual({ ideal: 1920 });
		expect(video.height).toEqual({ ideal: 1080 });
		expect(video.advanced).toEqual([{ focusMode: 'continuous' }]);
	});

	it('turns on continuous focus when the camera lists the mode', async () => {
		const apply = vi.fn(async () => {});
		const t = track({ focusMode: ['manual', 'continuous'] }, apply);

		expect(await applyContinuousFocus(t)).toBe(true);
		expect(apply).toHaveBeenCalledWith({ advanced: [{ focusMode: 'continuous' }] });
	});

	it('leaves a camera alone when it does not expose focus modes', async () => {
		const apply = vi.fn(async () => {});
		const t = track({ zoom: { min: 1, max: 4 } }, apply);

		expect(await applyContinuousFocus(t)).toBe(false);
		expect(apply).not.toHaveBeenCalled();
	});

	it('survives a camera that refuses the constraint', async () => {
		const apply = vi.fn(async () => {
			throw new Error('Overconstrained');
		});

		expect(await applyContinuousFocus(track({ focusMode: ['continuous'] }, apply))).toBe(false);
	});
});
