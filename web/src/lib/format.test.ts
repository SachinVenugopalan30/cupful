// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { formatGrams, formatHeight, formatKcal, formatWeight, heightToCm, weightToKg } from './format';

describe('formatKcal', () => {
	it('rounds half away from zero, like the server', () => {
		expect(formatKcal(1234.5)).toBe('1235');
		expect(formatKcal(-12.5)).toBe('-13');
		expect(formatKcal(0.4)).toBe('0');
		expect(formatKcal(-0.4)).toBe('0');
	});
});

describe('formatGrams', () => {
	it('shows one decimal, dropping a trailing .0', () => {
		expect(formatGrams(12.34)).toBe('12.3');
		expect(formatGrams(12.25)).toBe('12.3');
		expect(formatGrams(150)).toBe('150');
		expect(formatGrams(0.04)).toBe('0');
	});
});

describe('weight', () => {
	it('formats in the user unit', () => {
		expect(formatWeight(81.4, 'kg')).toBe('81.4 kg');
		expect(formatWeight(80, 'lb')).toBe('176.4 lb');
	});
	it('converts back to kg', () => {
		expect(weightToKg(81.4, 'kg')).toBe(81.4);
		expect(weightToKg(176.4, 'lb')).toBeCloseTo(80.01, 2);
	});
});

describe('height', () => {
	it('formats cm and feet/inches', () => {
		expect(formatHeight(178.4, 'cm')).toBe('178 cm');
		expect(formatHeight(177.8, 'ftin')).toBe('5′ 10″');
		expect(formatHeight(182.6, 'ftin')).toBe('6′ 0″'); // 71.9 in rounds up to 6 ft
	});
	it('converts feet and inches to cm', () => {
		expect(heightToCm(5, 10)).toBeCloseTo(177.8, 1);
	});
});
