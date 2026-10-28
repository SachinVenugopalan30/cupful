// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Estimate, EstimateItem, Meal } from '#lib/api.ts';
import {
	blankItem,
	choose,
	dayTotals,
	draftFromEstimate,
	draftFromMeal,
	fitWithin,
	itemReady,
	mealFromDraft,
	repeatMeal,
	sumItems,
	withGrams,
	withValue
} from './meal';

const UUIDV7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7/;

const rice: EstimateItem = {
	name: 'rice',
	portion_text: '1 small cup',
	grams: 150,
	kcal: 195,
	protein_g: 4,
	carbs_g: 42,
	fat_g: 0.4,
	origin: 'food_db',
	food_id: 1,
	food_name: 'Rice, white, cooked',
	match: 'matched',
	candidates: []
};
const fish: EstimateItem = {
	name: 'fish',
	portion_text: '1 piece',
	grams: 120,
	kcal: 170,
	protein_g: 26,
	carbs_g: 2,
	fat_g: 7,
	origin: 'ai_estimate',
	food_id: null,
	food_name: null,
	match: 'ambiguous',
	candidates: [
		{ food_id: 7, name: 'Fish curry', kcal: 144, protein_g: 14, carbs_g: 4, fat_g: 8 },
		{ food_id: 8, name: 'Fish, fried', kcal: 240, protein_g: 24, carbs_g: 9, fat_g: 12 }
	]
};
const durian: EstimateItem = {
	...rice,
	name: 'durian',
	kcal: null,
	protein_g: null,
	carbs_g: null,
	fat_g: null,
	origin: 'ai_estimate',
	food_id: null,
	food_name: null,
	match: 'unmatched'
};
const est: Estimate = {
	items: [rice, fish, durian],
	assumptions: 'One piece of fish.',
	confidence: 'low',
	provider_id: 3,
	warnings: ['Dropped "water": no amount.']
};
const ctx = { date: '2026-10-28', meal_type: 'lunch' as const, description: 'fish and rice' };

describe('fitWithin', () => {
	it('downscales the long side to the limit, keeping the aspect ratio', () => {
		expect(fitWithin(4032, 3024, 1600)).toEqual({ width: 1600, height: 1200 });
		expect(fitWithin(3024, 4032, 1600)).toEqual({ width: 1200, height: 1600 });
		expect(fitWithin(5000, 1, 1600)).toEqual({ width: 1600, height: 1 });
	});
	it('never upscales', () => {
		expect(fitWithin(800, 600, 1600)).toEqual({ width: 800, height: 600 });
	});
});

describe('draftFromEstimate', () => {
	const d = draftFromEstimate(est, ctx);
	it('carries the meal context and estimate metadata', () => {
		expect(d).toMatchObject({
			date: '2026-10-28',
			meal_type: 'lunch',
			description: 'fish and rice',
			scale: 1,
			source: 'ai',
			provider_id: 3,
			assumptions: 'One piece of fish.',
			confidence: 'low',
			warnings: ['Dropped "water": no amount.']
		});
		expect(d.id).toMatch(UUIDV7);
		expect(d.items.map((i) => i.id).every((id) => UUIDV7.test(id))).toBe(true);
	});
	it('flags items without numbers', () => {
		expect(d.items.map(itemReady)).toEqual([true, true, false]);
		expect(d.items[2].editable).toBe(true);
	});
	it('offers the estimate and each candidate for an ambiguous item', () => {
		expect(d.items[1].options.map((o) => o.name)).toEqual(['fish', 'Fish curry', 'Fish, fried']);
		expect(d.items[1].chosen).toBe(0);
		expect(d.items[0].options).toEqual([]);
	});
});

describe('item edits', () => {
	const [r, f, du] = draftFromEstimate(est, ctx).items;

	it('rescales numbers proportionally with grams', () => {
		const r2 = withGrams(r, 300);
		expect(r2).toMatchObject({ grams: 300, kcal: 390, protein_g: 8, carbs_g: 84, fat_g: 0.8 });
	});
	it('keeps null numbers null when grams change', () => {
		expect(withGrams(du, 50)).toMatchObject({ grams: 50, kcal: null });
	});
	it('swaps in a candidate, scaled to the current grams', () => {
		const f2 = choose(withGrams(f, 240), 1);
		expect(f2).toMatchObject({
			chosen: 1,
			name: 'Fish curry',
			food_id: 7,
			origin: 'food_db',
			grams: 240,
			kcal: 288,
			protein_g: 28
		});
		const back = choose(f2, 0);
		expect(back).toMatchObject({ name: 'fish', food_id: null, origin: 'ai_estimate', kcal: 340 });
	});
	it('sets a number by hand', () => {
		const d2 = withValue(withValue(withValue(withValue(du, 'kcal', 160), 'protein_g', 2), 'carbs_g', 30), 'fat_g', 5);
		expect(itemReady(d2)).toBe(true);
		expect(withValue(d2, 'kcal', null).kcal).toBeNull();
	});
	it('a blank item needs every number', () => {
		const b = blankItem();
		expect(b).toMatchObject({ name: '', grams: null, kcal: null, editable: true, origin: 'ai_estimate' });
		expect(itemReady({ ...b, name: 'x', kcal: 1, protein_g: 0, carbs_g: 0, fat_g: 0 })).toBe(true);
		expect(itemReady({ ...b, name: ' ', kcal: 1, protein_g: 0, carbs_g: 0, fat_g: 0 })).toBe(false);
	});
});

describe('totals', () => {
	it('sums items, counting missing numbers as zero, times the multiplier', () => {
		const d = draftFromEstimate(est, ctx);
		expect(sumItems(d.items, 1)).toEqual({ kcal: 365, protein_g: 30, carbs_g: 44, fat_g: 7.4 });
		expect(sumItems(d.items, 2).kcal).toBe(730);
	});
	it('sums meals for the day', () => {
		const t = { kcal: 100, protein_g: 1, carbs_g: 2, fat_g: 3 };
		expect(dayTotals([{ totals: t }, { totals: t }] as Meal[])).toEqual({
			kcal: 200,
			protein_g: 2,
			carbs_g: 4,
			fat_g: 6
		});
	});
});

describe('mealFromDraft', () => {
	it('applies the multiplier to items before saving and keeps scale for display', () => {
		const d = draftFromEstimate({ ...est, items: [rice, fish] }, ctx);
		d.scale = 1.5;
		const m = mealFromDraft(d);
		expect(m.scale).toBe(1.5);
		expect(m.source).toBe('ai');
		expect(m.items[0]).toMatchObject({ grams: 225, kcal: 292.5, food_id: 1, origin: 'food_db' });
		expect(m.items[1]).toMatchObject({ grams: 180, kcal: 255, origin: 'ai_estimate' });
		expect(m.items[0]).not.toHaveProperty('options');
		expect(Object.keys(m.items[0]).sort()).toEqual(
			['carbs_g', 'fat_g', 'food_id', 'grams', 'id', 'kcal', 'name', 'origin', 'portion_text', 'protein_g'].sort()
		);
	});
	it('refuses items that still need numbers', () => {
		expect(() => mealFromDraft(draftFromEstimate(est, ctx))).toThrow(/needs/);
	});
});

const saved: Meal = {
	id: '0190e000-0000-7000-8000-000000000001',
	date: '2026-10-27',
	meal_type: 'dinner',
	description: 'rice',
	scale: 2,
	assumptions: null,
	confidence: 'high',
	source: 'ai',
	provider_id: 3,
	updated_at: 0,
	items: [
		{
			id: '0190e000-0000-7000-8000-000000000002',
			name: 'Rice',
			portion_text: '1 cup',
			grams: 300,
			kcal: 390,
			protein_g: 8,
			carbs_g: 84,
			fat_g: 0.8,
			food_id: 1,
			origin: 'food_db'
		}
	],
	totals: { kcal: 390, protein_g: 8, carbs_g: 84, fat_g: 0.8 }
};

describe('draftFromMeal (edit)', () => {
	it('reopens with the same ids and the multiplier undone on items', () => {
		const d = draftFromMeal(saved);
		expect(d.id).toBe(saved.id);
		expect(d.scale).toBe(2);
		expect(d.items[0]).toMatchObject({ id: saved.items[0].id, grams: 150, kcal: 195 });
		// re-saving unchanged gives the same stored values
		expect(mealFromDraft(d).items[0]).toMatchObject({ grams: 300, kcal: 390 });
	});
});

describe('repeatMeal', () => {
	it('copies the full meal to a date with new ids and source repeat', () => {
		const m = repeatMeal(saved, '2026-10-28');
		expect(m.id).not.toBe(saved.id);
		expect(m.id).toMatch(UUIDV7);
		expect(m.items[0].id).not.toBe(saved.items[0].id);
		expect(m).toMatchObject({ date: '2026-10-28', meal_type: 'dinner', source: 'repeat', scale: 2, provider_id: null });
		expect(m.items[0]).toMatchObject({ grams: 300, kcal: 390, food_id: 1 });
	});
});
