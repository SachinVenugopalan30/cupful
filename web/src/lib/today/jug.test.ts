// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { jugFraction, jugLabel, jugMarks } from './jug.ts';

describe('jugFraction', () => {
	it('is eaten over budget', () => {
		expect(jugFraction(500, 2000)).toBe(0.25);
		expect(jugFraction(0, 2000)).toBe(0);
	});
	it('clamps over budget to a full jug', () => {
		expect(jugFraction(2500, 2000)).toBe(1);
	});
	it('is empty without a budget or with bad numbers', () => {
		expect(jugFraction(300, 0)).toBe(0);
		expect(jugFraction(-5, 2000)).toBe(0);
		expect(jugFraction(Number.NaN, 2000)).toBe(0);
		expect(jugFraction(10, Number.NaN)).toBe(0);
	});
});

describe('jugMarks', () => {
	it('puts four labels at quarters of the budget, top first', () => {
		expect(jugMarks(2002).map((m) => m.label)).toEqual(['2000', '1500', '1000', '500']);
		expect(jugMarks(2000).map((m) => m.at)).toEqual([1, 0.75, 0.5, 0.25]);
	});
	it('rounds labels to the nearest 50', () => {
		expect(jugMarks(1807).map((m) => m.label)).toEqual(['1800', '1350', '900', '450']);
	});
	it('has no marks without a budget', () => {
		expect(jugMarks(0)).toEqual([]);
	});
});

describe('jugLabel', () => {
	it('states eaten and plan', () => {
		expect(jugLabel(1239, 2002, false)).toBe('1,239 of 2,002 kcal plan eaten');
		expect(jugLabel(1239, 2002, true)).toBe('1,239 of 2,002 kcal goal eaten');
	});
	it('says over when over', () => {
		expect(jugLabel(2300, 2000, false)).toBe('2,300 of 2,000 kcal plan eaten, over budget');
	});
});
