// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { countValue, easeOutCubic, startTween, tweenValue } from './count-up.ts';

describe('count-up', () => {
	it('eases out from 0 to 1 and clamps', () => {
		expect(easeOutCubic(0)).toBe(0);
		expect(easeOutCubic(1)).toBe(1);
		expect(easeOutCubic(0.5)).toBeCloseTo(0.875);
		expect(easeOutCubic(-1)).toBe(0);
		expect(easeOutCubic(2)).toBe(1);
	});
	it('interpolates between the old and new headline', () => {
		expect(countValue(0, 1000, 0)).toBe(0);
		expect(countValue(0, 1000, 1)).toBe(1000);
		expect(countValue(1000, 2000, 0.5)).toBeCloseTo(1875);
	});
});

describe('count on change', () => {
	it('tweens from the shown value to the new one over the duration', () => {
		const t = startTween(120, 480, 1000, 520, false);
		expect(tweenValue(t, 1000)).toBe(120);
		expect(tweenValue(t, 1520)).toBe(480);
		expect(tweenValue(t, 9999)).toBe(480);
		const mid = tweenValue(t, 1260);
		expect(mid).toBeGreaterThan(120);
		expect(mid).toBeLessThan(480);
	});
	it('counts down as well as up', () => {
		const t = startTween(900, 300, 0, 520, false);
		expect(tweenValue(t, 260)).toBeLessThan(900);
		expect(tweenValue(t, 260)).toBeGreaterThan(300);
	});
	it('retargets from wherever the last count had got to', () => {
		const first = startTween(0, 1000, 0, 520, false);
		const shown = tweenValue(first, 200);
		const second = startTween(shown, 400, 200, 520, false);
		expect(tweenValue(second, 200)).toBe(shown);
		expect(tweenValue(second, 720)).toBe(400);
	});
	it('snaps straight to the value under reduced motion', () => {
		const t = startTween(10, 77, 5, 520, true);
		expect(tweenValue(t, 5)).toBe(77);
	});
});
