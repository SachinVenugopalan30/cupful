// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { TrendDay } from '../api';
import { stepsTile, weekBars, weekRange } from './week.ts';

const day = (date: string, eaten: number | null, budget: number | null = 2000): TrendDay => ({
	date,
	eaten_kcal: eaten,
	budget_kcal: budget,
	weight_kg: null,
	steps: null,
	gym_minutes: null
});

describe('weekRange', () => {
	it('is the seven days ending today', () => {
		expect(weekRange('2026-11-12')).toEqual({ from: '2026-11-06', to: '2026-11-12' });
	});
});

describe('weekBars', () => {
	const days = [
		day('2026-11-06', 1830),
		day('2026-11-07', 2300),
		day('2026-11-08', null),
		day('2026-11-10', 1700)
	];

	it('returns seven bars, oldest first, with weekday labels', () => {
		const bars = weekBars(days, '2026-11-12', 700, 2000);
		expect(bars).toHaveLength(7);
		expect(bars.map((b) => b.label)).toEqual(['Fri', 'Sat', 'Sun', 'Mon', 'Tue', 'Wed', 'Thu']);
		expect(bars[6].today).toBe(true);
		expect(bars.filter((b) => b.today)).toHaveLength(1);
	});

	it('follows the live eaten figure for today, not the server row', () => {
		const bars = weekBars([...days, day('2026-11-12', 100)], '2026-11-12', 900, 2000);
		expect(bars[6].kcal).toBe(900);
	});

	it('leaves a day with nothing logged empty and flags a day over its budget', () => {
		const bars = weekBars(days, '2026-11-12', 0, 2000);
		expect(bars[2].kcal).toBeNull();
		expect(bars[2].height).toBe(0);
		expect(bars[1].over).toBe(true);
		expect(bars[0].over).toBe(false);
		// a missing row (09 not in the list) is empty too
		expect(bars[3].kcal).toBeNull();
	});

	it('scales heights to the taller of the plan line and the biggest day', () => {
		const bars = weekBars(days, '2026-11-12', 700, 2000);
		const top = Math.max(...bars.map((b) => b.height));
		expect(top).toBeLessThanOrEqual(1);
		const scale = Math.max(2000 * 1.25, ...bars.map((b) => b.kcal ?? 0));
		expect(bars[1].height).toBeCloseTo(2300 / scale, 5);
		expect(bars[0].planAt).toBeCloseTo(2000 / scale, 5);
	});

	it('judges an old day against its own budget when it has one', () => {
		const bars = weekBars([day('2026-11-06', 1900, 1800)], '2026-11-12', 0, 2000);
		expect(bars[0].over).toBe(true);
	});
});

describe('stepsTile', () => {
	it('gives the fraction of typical and the words', () => {
		expect(stepsTile(6240, 8000)).toEqual({ value: 0.78, label: 'of 8,000 typical' });
	});
	it('caps the meter at full but keeps the words', () => {
		expect(stepsTile(12000, 8000).value).toBe(1);
	});
	it('has no meter without a typical figure', () => {
		expect(stepsTile(5000, 0)).toEqual({ value: 0, label: 'steps' });
	});
});
