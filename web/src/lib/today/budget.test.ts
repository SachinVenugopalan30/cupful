// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Budget, BudgetFlag, Meal } from '../api';
import { breakdown, eaten, flagMessages, leftOver, macroRows } from './budget';

const budget: Budget = {
	date: '2026-10-28',
	age: 36,
	weight_kg: 80,
	steps: 8000,
	met: 5,
	bmr: 1755,
	base: 2106,
	steps_kcal: 251,
	gym_kcal: 0,
	maintenance: 2357,
	deficit: 550,
	floor: 1500,
	budget: 1807,
	protein_g: 128,
	fat_g: 50.2,
	carbs_g: 213.5,
	flags: []
};

const meal = (kcal: number, protein_g: number, carbs_g: number, fat_g: number) =>
	({ totals: { kcal, protein_g, carbs_g, fat_g } }) as Meal;

describe('flagMessages', () => {
	it('has plain words for every flag', () => {
		const all: BudgetFlag[] = [
			'provisional',
			'deficit_capped',
			'floor_applied',
			'floor_above_maintenance',
			'macros_overcommitted'
		];
		const msgs = flagMessages(all);
		expect(msgs).toHaveLength(5);
		expect(msgs[0]).toMatch(/no steps logged yet/i);
		expect(msgs[3]).toMatch(/no deficit/i);
		expect(msgs[4]).toMatch(/protein takes up most/i);
		for (const m of msgs) expect(m.length).toBeGreaterThan(20);
	});
	it('says nothing for no flags', () => {
		expect(flagMessages([])).toEqual([]);
	});
});

describe('eaten and left/over', () => {
	it('sums meal totals', () => {
		expect(eaten([meal(500, 30, 50, 20), meal(250.4, 10, 20.5, 5)])).toEqual({
			kcal: 750.4,
			protein_g: 40,
			carbs_g: 70.5,
			fat_g: 25
		});
		expect(eaten([])).toEqual({ kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 });
	});
	it('reports left, exactly on budget as 0 left, and over', () => {
		expect(leftOver(1000, 1807)).toEqual({ over: false, kcal: 807 });
		expect(leftOver(1807.2, 1807)).toEqual({ over: false, kcal: 0 });
		expect(leftOver(2000, 1807)).toEqual({ over: true, kcal: 193 });
	});
});

describe('macroRows', () => {
	it('gives meter values against each target', () => {
		const rows = macroRows(budget, { kcal: 0, protein_g: 64, carbs_g: 213.5, fat_g: 60.24 });
		expect(rows.map((r) => r.key)).toEqual(['protein_g', 'carbs_g', 'fat_g']);
		expect(rows[0].value).toBeCloseTo(0.5);
		expect(rows[1]).toMatchObject({ value: 1, over: false });
		expect(rows[2].over).toBe(true);
		expect(rows[2].value).toBeCloseTo(1.2);
	});
	it('never shows a bare 0 g carbs target when macros are over-committed', () => {
		const b = { ...budget, carbs_g: 0, fat_g: 20, flags: ['macros_overcommitted'] as BudgetFlag[] };
		const rows = macroRows(b, { kcal: 0, protein_g: 0, carbs_g: 10, fat_g: 0 });
		const carbs = rows.find((r) => r.key === 'carbs_g')!;
		expect(carbs.target).toBeNull();
		expect(carbs.value).toBe(0);
		expect(carbs.over).toBe(false);
		expect(rows.find((r) => r.key === 'fat_g')!.target).toBe(20);
	});
});

describe('breakdown', () => {
	it('lists every term the API returns, with an explanation', () => {
		const terms = breakdown(budget);
		const labels = terms.map((t) => t.label).join(' ');
		for (const word of ['BMR', 'Base', 'Steps', 'Gym', 'Maintenance', 'Deficit', 'Floor', 'Budget', 'Weight', 'Age', 'MET'])
			expect(labels).toContain(word);
		for (const t of terms) expect(t.note.length).toBeGreaterThan(10);
		expect(terms.find((t) => t.label === 'Steps used')!.value).toBe('8,000');
	});
});
