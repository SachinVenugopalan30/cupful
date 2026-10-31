// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { TrendDay } from '../api';
import {
	averages,
	consistency,
	consistencyColumns,
	formatNumber,
	goalRemaining,
	intakeReadout,
	loadRange,
	mealShares,
	rangeDates,
	rollingAverage,
	saveRange,
	shortDate,
	signedWeightDelta,
	stepsStats,
	stepsReadout,
	tickDates,
	weightStats,
	weightReadout
} from './logic';

const d = (date: string, o: Partial<TrendDay> = {}): TrendDay => ({
	date,
	eaten_kcal: null,
	budget_kcal: null,
	weight_kg: null,
	steps: null,
	gym_minutes: null,
	...o
});

describe('rangeDates', () => {
	it('ends today and spans exactly n days', () => {
		expect(rangeDates(7, '2026-10-29')).toEqual({ from: '2026-10-23', to: '2026-10-29' });
		expect(rangeDates(30, '2026-03-01').from).toBe('2026-01-31');
		expect(rangeDates(365, '2026-10-29').from).toBe('2025-10-30');
	});
});

describe('rollingAverage', () => {
	it('averages the values present in the trailing window, skipping gaps', () => {
		expect(rollingAverage([80, null, 82, null, null], 3)).toEqual([80, 80, 81, 82, 82]);
	});
	it('is null until a value has been seen, and when the window is empty', () => {
		expect(rollingAverage([null, 70, null, null, null], 2)).toEqual([null, 70, 70, null, null]);
	});
	it('drops values that leave the window', () => {
		expect(rollingAverage([60, 70, 80], 2)).toEqual([60, 65, 75]);
	});
});

describe('averages', () => {
	it('uses only days with meals, for intake and for the budget it is compared with', () => {
		const days = [
			d('a', { eaten_kcal: 2000, budget_kcal: 2200 }),
			d('b', { budget_kcal: 9999 }), // nothing logged: not a zero-calorie day
			d('c', { eaten_kcal: 1600, budget_kcal: 2000 }),
			d('d', { eaten_kcal: 1800 }) // before the first profile: no budget
		];
		expect(averages(days)).toMatchObject({ days: 3, intake: 1800, budget: 2100 });
	});
	it('is null when nothing is logged', () => {
		expect(averages([d('a')])).toEqual({ days: 0, intake: null, budget: null, steps: null });
	});
	it('averages steps over days that have them', () => {
		expect(averages([d('a', { steps: 4000 }), d('b'), d('c', { steps: 8000 })]).steps).toBe(6000);
	});
});

describe('tickDates', () => {
	it('keeps about n evenly spaced dates, always the last', () => {
		const dates = Array.from({ length: 30 }, (_, i) => `x${i}`);
		const t = tickDates(dates, 5);
		expect(t.length).toBeLessThanOrEqual(5);
		expect(t.at(-1)).toBe('x29');
		expect(t[0]).toBe('x0');
	});
	it('keeps all when few', () => {
		expect(tickDates(['a', 'b'], 5)).toEqual(['a', 'b']);
	});
});

describe('readouts', () => {
	const today = '2026-10-29';
	it('intake says left or over in words', () => {
		expect(intakeReadout(d('2026-10-29', { eaten_kcal: 1820, budget_kcal: 2100 }), today)).toBe(
			'Today: 1820 kcal eaten of 2100 kcal, 280 left.'
		);
		expect(intakeReadout(d('2026-10-28', { eaten_kcal: 2300, budget_kcal: 2100 }), today)).toBe(
			'Yesterday: 2300 kcal eaten of 2100 kcal, 200 over.'
		);
	});
	it('intake handles no meals and no budget', () => {
		expect(intakeReadout(d('2026-10-29'), today)).toBe('Today: no meals logged.');
		expect(intakeReadout(d('2026-10-29', { eaten_kcal: 900 }), today)).toBe('Today: 900 kcal eaten.');
	});
	it('weight uses the display unit and says what is missing', () => {
		expect(weightReadout(d('2026-10-29', { weight_kg: 80 }), 80.5, today, 'kg')).toBe(
			'Today: 80 kg weighed in, 80.5 kg 7-day average.'
		);
		expect(weightReadout(d('2026-10-29'), null, today, 'kg')).toBe('Today: no weigh-in.');
	});
	it('steps mention gym minutes only when there are some', () => {
		expect(stepsReadout(d('2026-10-29', { steps: 9000, gym_minutes: 45 }), today)).toBe(
			'Today: 9000 steps, gym 45 min.'
		);
		expect(stepsReadout(d('2026-10-29'), today)).toBe('Today: no steps recorded.');
	});
});

describe('remembered range', () => {
	const store = (init?: string) => {
		const m = new Map<string, string>(init ? [['cupful-trends-range', init]] : []);
		return {
			getItem: (k: string) => m.get(k) ?? null,
			setItem: (k: string, v: string) => void m.set(k, v)
		} as Storage;
	};
	it('defaults to 30 and ignores junk', () => {
		expect(loadRange(store())).toBe(30);
		expect(loadRange(store('12'))).toBe(30);
	});
	it('round-trips', () => {
		const s = store();
		saveRange(90, s);
		expect(loadRange(s)).toBe(90);
	});
	it('survives storage that throws', () => {
		const bad = {
			getItem: () => {
				throw new Error('x');
			},
			setItem: () => {
				throw new Error('x');
			}
		} as unknown as Storage;
		expect(loadRange(bad)).toBe(30);
		expect(() => saveRange(7, bad)).not.toThrow();
	});
});

describe('numbers and dates', () => {
	it('groups thousands', () => {
		expect(formatNumber(1762)).toBe('1,762');
		expect(formatNumber(999)).toBe('999');
		expect(formatNumber(1234567)).toBe('1,234,567');
	});
	it('labels axes as short dates', () => {
		expect(shortDate('2026-10-02')).toBe('Oct 2');
		expect(shortDate('2026-01-31')).toBe('Jan 31');
	});
});

describe('consistency', () => {
	it('counts within, over, no log and logged-but-unscored days', () => {
		const days = [
			d('a', { eaten_kcal: 2000, budget_kcal: 2200 }),
			d('b', { eaten_kcal: 2300, budget_kcal: 2200 }),
			d('c'),
			d('d', { eaten_kcal: 1800 }) // before any profile: no budget
		];
		const c = consistency(days);
		expect(c.cells.map((x) => x.kind)).toEqual(['in', 'over', 'none', 'unknown']);
		expect(c).toMatchObject({ in: 1, over: 1, none: 1, unknown: 1, logged: 3 });
	});
	it('treats an exact budget as within', () => {
		expect(consistency([d('a', { eaten_kcal: 2200, budget_kcal: 2200 })]).in).toBe(1);
	});
	it('caps the grid at 30 columns', () => {
		expect(consistencyColumns(7)).toBe(7);
		expect(consistencyColumns(30)).toBe(30);
		expect(consistencyColumns(365)).toBe(30);
		expect(consistencyColumns(0)).toBe(1);
	});
});

describe('weight', () => {
	const days = [
		d('2026-09-20', { weight_kg: 81 }),
		d('2026-09-25', { weight_kg: 80.5 }),
		d('2026-10-01', { weight_kg: 80.2 }),
		d('2026-10-15', { weight_kg: 79.8 }),
		d('2026-10-29', { weight_kg: 78.8 })
	];
	it('reports the range change and the trailing month change', () => {
		const w = weightStats(days);
		expect(w.count).toBe(5);
		expect(w.change).toBeCloseTo(-2.2);
		expect(w.monthChange).toBeCloseTo(-1.4); // Oct 1 to Oct 29
		expect(w.latest).toBe(78.8);
	});
	it('has no change with one weigh-in', () => {
		expect(weightStats([d('2026-10-01', { weight_kg: 80 })]).change).toBe(null);
	});
	it('signs a change with the render minus sign and the display unit', () => {
		expect(signedWeightDelta(-1.4, 'kg')).toBe('−1.4 kg');
		expect(signedWeightDelta(0.5, 'kg')).toBe('+0.5 kg');
		expect(signedWeightDelta(0, 'kg')).toBe('0 kg');
		expect(signedWeightDelta(-0.45359237, 'lb')).toBe('−1 lb');
	});
	it('says what is left to the goal', () => {
		expect(goalRemaining(79.4, 75, 'kg')).toBe('4.4 kg to goal');
		expect(goalRemaining(74, 75, 'kg')).toBe('Goal reached');
		expect(goalRemaining(79.4, null, 'kg')).toBe(null);
	});
});

describe('steps', () => {
	it('averages the recorded days and finds the best', () => {
		const s = stepsStats(
			[d('a', { steps: 6000 }), d('b'), d('c', { steps: 10000 })],
			8000
		);
		expect(s.count).toBe(2);
		expect(s.avg).toBe(8000);
		expect(s.best).toBe(10000);
		expect(s.diff).toBe(0);
	});
	it('has no average without steps', () => {
		expect(stepsStats([d('a')], 8000)).toMatchObject({ count: 0, avg: null, best: null });
	});
});

describe('mealShares', () => {
	it('turns average kcal into whole percentages that add to 100', () => {
		const shares = mealShares({ breakfast: 388, lunch: 670, dinner: 528, snack: 176 })!;
		expect(shares.map((s) => s.pct)).toEqual([22, 38, 30, 10]);
		expect(shares.reduce((a, s) => a + s.pct, 0)).toBe(100);
		expect(shares.map((s) => s.label)).toEqual(['Breakfast', 'Lunch', 'Dinner', 'Snack']);
	});
	it('is null for no mix', () => {
		expect(mealShares(null)).toBe(null);
		expect(mealShares({ breakfast: 0, lunch: 0, dinner: 0, snack: 0 })).toBe(null);
	});
});
