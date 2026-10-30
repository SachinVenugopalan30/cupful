// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it, vi } from 'vitest';
import { cameraConstraints, nearFocusDistance, startFocus } from './scanner.ts';

function track(caps: unknown, apply = vi.fn(async (_: MediaTrackConstraints) => {})) {
	return { getCapabilities: () => caps, applyConstraints: apply } as unknown as MediaStreamTrack;
}

const noWait = async () => {};

describe('camera setup', () => {
	it('asks for the rear camera at 1080p with continuous focus', () => {
		const video = cameraConstraints().video as MediaTrackConstraints;
		expect(video.facingMode).toEqual({ ideal: 'environment' });
		expect(video.width).toEqual({ ideal: 1920 });
		expect(video.height).toEqual({ ideal: 1080 });
		expect(video.advanced).toEqual([{ focusMode: 'continuous' }]);
	});
});

describe('nearFocusDistance', () => {
	it('is 20 cm, clamped into what the camera allows', () => {
		expect(nearFocusDistance({ min: 0.1, max: 5 })).toBe(0.2);
		expect(nearFocusDistance({ min: 0.3, max: 5 })).toBe(0.3);
		expect(nearFocusDistance({ min: 0.05, max: 0.1 })).toBe(0.1);
	});
});

describe('startFocus', () => {
	const caps = { focusMode: ['manual', 'continuous'], focusDistance: { min: 0.1, max: 5, step: 0.01 } };

	it('moves the lens near, then hands over to continuous autofocus', async () => {
		const apply = vi.fn(async (_: MediaTrackConstraints) => {});
		expect(await startFocus(track(caps, apply), noWait)).toBe('auto');
		expect(apply.mock.calls.map((c) => c[0])).toEqual([
			{ advanced: [{ focusMode: 'manual', focusDistance: 0.2 }] },
			{ advanced: [{ focusMode: 'continuous' }] }
		]);
	});

	it('goes straight to continuous when the distance cannot be set', async () => {
		const apply = vi.fn(async (_: MediaTrackConstraints) => {});
		expect(await startFocus(track({ focusMode: ['continuous'] }, apply), noWait)).toBe('auto');
		expect(apply.mock.calls.map((c) => c[0])).toEqual([{ advanced: [{ focusMode: 'continuous' }] }]);
	});

	it('falls back to a fixed near focus when continuous is not offered', async () => {
		const apply = vi.fn(async (_: MediaTrackConstraints) => {});
		const t = track({ focusMode: ['manual'], focusDistance: { min: 0.1, max: 5 } }, apply);
		expect(await startFocus(t, noWait)).toBe('near');
		expect(apply).toHaveBeenLastCalledWith({ advanced: [{ focusMode: 'manual', focusDistance: 0.2 }] });
	});

	it('leaves a camera alone when it exposes no focus control', async () => {
		const apply = vi.fn(async (_: MediaTrackConstraints) => {});
		expect(await startFocus(track({ zoom: { min: 1, max: 4 } }, apply), noWait)).toBe('none');
		expect(apply).not.toHaveBeenCalled();
		expect(await startFocus({} as MediaStreamTrack, noWait)).toBe('none');
	});

	it('survives a camera that refuses the constraints', async () => {
		const apply = vi.fn(async (_: MediaTrackConstraints) => {
			throw new Error('Overconstrained');
		});
		expect(await startFocus(track(caps, apply), noWait)).toBe('none');
	});
});
