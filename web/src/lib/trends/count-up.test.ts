// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { countValue, easeOutCubic } from './count-up';

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
