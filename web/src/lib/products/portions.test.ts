// SPDX-License-Identifier: AGPL-3.0-or-later
// Household portions: the picker's options, de-duplication, labels and grams.
import { describe, expect, it } from 'vitest';
import { formatQuantity, portionGrams, portionLabel, portionOptions, bestPortion } from './portions';

const p = (unit: string, grams: number) => ({ unit, grams });

describe('portionOptions', () => {
	it('drops unusable rows', () => {
		expect(portionOptions([p('cup', 0), p(' ', 40), p('tsp', -1), p('scoop', 31)])).toEqual([p('scoop', 31)]);
	});

	it('sorts cup, tbsp, tsp, scoop, serving, then other units', () => {
		expect(portionOptions([p('biscuit', 12), p('serving', 40), p('scoop', 31), p('tsp', 5), p('tbsp', 15), p('cup', 80)])).toEqual([
			p('cup', 80),
			p('tbsp', 15),
			p('tsp', 5),
			p('scoop', 31),
			p('serving', 40),
			p('biscuit', 12)
		]);
	});

	it('de-duplicates units with the same grams, preferring a named unit over serving', () => {
		expect(portionOptions([p('serving', 31), p('scoop', 31)])).toEqual([p('scoop', 31)]);
		expect(portionOptions([p('biscuit', 12), p('serving', 12)])).toEqual([p('biscuit', 12)]);
		expect(portionOptions([p('cup', 40), p('serving', 40.04)])).toEqual([p('cup', 40)]);
	});

	it('picks the best one-unit portion first', () => {
		expect(bestPortion([p('serving', 40), p('cup', 80)])).toEqual(p('cup', 80));
		expect(bestPortion([])).toBeNull();
	});
});

describe('formatQuantity', () => {
	it('shows simple fractions', () => {
		expect(formatQuantity(0.5)).toBe('½');
		expect(formatQuantity(0.25)).toBe('¼');
		expect(formatQuantity(0.75)).toBe('¾');
		expect(formatQuantity(1.5)).toBe('1½');
		expect(formatQuantity(2)).toBe('2');
		expect(formatQuantity(0.1)).toBe('0.1');
	});
});

describe('portionLabel', () => {
	it('shows the count, unit and grams', () => {
		expect(portionLabel(0.5, p('cup', 80))).toBe('½ cup (40 g)');
		expect(portionLabel(2, p('tsp', 5))).toBe('2 tsp (10 g)');
		expect(portionLabel(1, p('scoop', 31))).toBe('1 scoop (31 g)');
		expect(portionLabel(2, p('scoop', 31))).toBe('2 scoops (62 g)');
	});

	it('uses millilitres for liquids and keeps fl oz invariant', () => {
		expect(portionLabel(1, p('cup', 240), 'ml')).toBe('1 cup (240 ml)');
		expect(portionLabel(2, p('fl oz', 30), 'ml')).toBe('2 fl oz (60 ml)');
	});
});

describe('portionGrams', () => {
	it('multiplies quantity by grams per unit', () => {
		expect(portionGrams(2, p('tsp', 5))).toBe(10);
		expect(portionGrams(0.5, p('cup', 80))).toBe(40);
		expect(portionGrams(1.25, p('scoop', 31))).toBe(38.8);
	});
});
