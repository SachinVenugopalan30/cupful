// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { parseCount, parseWeight, sourceLabel, toggle } from './logic';

describe('parseCount', () => {
	it('empty clears the field', () => {
		expect(parseCount('', 150000, 'Steps')).toEqual({ value: null });
		expect(parseCount('  ', 150000, 'Steps')).toEqual({ value: null });
	});
	it('accepts whole numbers in range, including 0', () => {
		expect(parseCount('8000', 150000, 'Steps')).toEqual({ value: 8000 });
		expect(parseCount('0', 600, 'Gym minutes')).toEqual({ value: 0 });
	});
	it('rejects fractions, negatives, and values over the max, naming the field', () => {
		expect(parseCount('12.5', 600, 'Gym minutes')).toHaveProperty('error');
		expect(parseCount('-1', 600, 'Gym minutes')).toHaveProperty('error');
		expect(parseCount('700', 600, 'Gym minutes')).toEqual({ error: 'Gym minutes must be a whole number from 0 to 600.' });
	});
});

describe('parseWeight', () => {
	it('converts to kg from the display unit', () => {
		expect(parseWeight('80', 'kg')).toEqual({ value: 80 });
		const lb = parseWeight('176', 'lb');
		expect((lb as { value: number }).value).toBeCloseTo(79.83, 1);
	});
	it('clears when empty and rejects out of range (server: 20 to 400 kg)', () => {
		expect(parseWeight('', 'kg')).toEqual({ value: null });
		expect(parseWeight('10', 'kg')).toHaveProperty('error');
		expect(parseWeight('900', 'lb')).toHaveProperty('error');
		expect(parseWeight('abc', 'kg')).toHaveProperty('error');
	});
});

describe('toggle', () => {
	it('adds in canonical order and removes', () => {
		expect(toggle(['Legs'], 'Chest')).toEqual(['Chest', 'Legs']);
		expect(toggle(['Chest', 'Legs'], 'Chest')).toEqual(['Legs']);
	});
});

describe('sourceLabel', () => {
	it('names every source in words', () => {
		expect(sourceLabel('manual')).toBe('Manual');
		expect(sourceLabel('shortcut')).toBe('Shortcut');
		expect(sourceLabel('import')).toBe('Import');
		expect(sourceLabel('google')).toBe('Google');
	});
});
