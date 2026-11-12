// SPDX-License-Identifier: AGPL-3.0-or-later
// Review builder helpers: a product as a draft item, appending without a blank starter row, and
// the meal name carried through saving.
import { describe, expect, it } from 'vitest';
import type { Product } from '#lib/api.ts';
import {
	appendProduct,
	blankItem,
	draftFromMeal,
	draftItemFromProduct,
	manualDraft,
	mealFromDraft,
	type Draft
} from './meal';
import type { Meal } from '#lib/api.ts';

const UUIDV7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7/;

const product = {
	food_id: 42,
	name: 'Choc protein',
	brand: 'Acme',
	serving_g: 31,
	serving_unit: 'g',
	serving_text: null,
	kcal: 387.1,
	protein_g: 80,
	carbs_g: 6.5,
	fat_g: 4.8
} as Product;

const ctx = { date: '2026-10-28', meal_type: 'lunch' as const, description: '' };

describe('draftItemFromProduct', () => {
	it('makes a food_db item with a new id at one serving', () => {
		const i = draftItemFromProduct(product);
		expect(i).toMatchObject({
			name: 'Choc protein',
			food_id: 42,
			origin: 'food_db',
			grams: 31,
			portion_text: '1 serving (31 g)',
			editable: false,
			options: [],
			chosen: 0
		});
		expect(i.id).toMatch(UUIDV7);
		expect(i.kcal).toBeCloseTo(120, 0);
		expect(draftItemFromProduct(product).id).not.toBe(i.id);
	});

	it('defaults to 100 g when there is no serving size', () => {
		const i = draftItemFromProduct({ ...product, serving_g: null });
		expect(i).toMatchObject({ grams: 100, portion_text: '100 g' });
		expect(i.kcal).toBeCloseTo(387.1, 0);
	});

	it('defaults to the best one-unit portion and labels it', () => {
		const i = draftItemFromProduct({
			...product,
			portions: [
				{ unit: 'serving', grams: 31 },
				{ unit: 'scoop', grams: 31 },
				{ unit: 'cup', grams: 80 }
			]
		});
		expect(i).toMatchObject({
			grams: 80,
			portion_text: '1 cup (80 g)',
			portion: { unit: 'cup', grams: 80 },
			quantity: 1
		});
		expect(i.portions).toEqual([
			{ unit: 'cup', grams: 80 },
			{ unit: 'scoop', grams: 31 }
		]);
	});

	it('falls back to grams when the product has no usable portion', () => {
		const i = draftItemFromProduct({ ...product, portions: [] });
		expect(i.portion).toBeNull();
		expect(i.portions).toEqual([]);
		expect(i).toMatchObject({ grams: 31, portion_text: '1 serving (31 g)' });
	});

	it('carries the product amount unit for liquids', () => {
		const milk = draftItemFromProduct({
			...product,
			name: 'Milk',
			serving_g: 240,
			serving_unit: 'ml',
			serving_text: '1 cup (240 ml)',
			portions: [{ unit: 'cup', grams: 240 }]
		});
		expect(milk).toMatchObject({ amount_unit: 'ml', grams: 240, portion_text: '1 cup (240 ml)' });
		expect(draftItemFromProduct(product).amount_unit).toBe('g');
	});
});

describe('appendProduct', () => {
	it('appends after existing items, keeping them', () => {
		const ai = { ...blankItem(), name: 'fish', kcal: 170, protein_g: 26, carbs_g: 2, fat_g: 7 };
		const next = appendProduct([ai], product);
		expect(next).toHaveLength(2);
		expect(next[0]).toBe(ai);
		expect(next[1]).toMatchObject({ name: 'Choc protein', origin: 'food_db' });
	});

	it('drops an untouched blank starter row', () => {
		const next = appendProduct([blankItem()], product);
		expect(next).toHaveLength(1);
		expect(next[0].name).toBe('Choc protein');
	});

	it('keeps a starter row the user touched', () => {
		const named = { ...blankItem(), name: 'banana' };
		expect(appendProduct([named], product)).toHaveLength(2);
		const grams = { ...blankItem(), grams: 80 };
		expect(appendProduct([grams], product)).toHaveLength(2);
		const numbers = { ...blankItem(), kcal: 50 };
		expect(appendProduct([numbers], product)).toHaveLength(2);
		const portion = { ...blankItem(), portion_text: '1 coffee' };
		expect(appendProduct([portion], product)).toHaveLength(2);
	});
});

describe('meal name', () => {
	it('saves the edited description', () => {
		const d = manualDraft(ctx);
		d.description = 'Lunch with friends';
		d.items = [draftItemFromProduct(product)];
		expect(mealFromDraft(d).description).toBe('Lunch with friends');
	});

	it('allows a blank name', () => {
		const d = manualDraft(ctx);
		d.items = [draftItemFromProduct(product)];
		expect(mealFromDraft(d).description).toBe('');
	});
});

describe('picker metadata', () => {
	it('is not saved with the meal', () => {
		const d = manualDraft(ctx) as Draft;
		d.items = [draftItemFromProduct({ ...product, portions: [{ unit: 'cup', grams: 80 }] })];
		const item = mealFromDraft(d).items[0] as unknown as Record<string, unknown>;
		expect(item).not.toHaveProperty('portions');
		expect(item).not.toHaveProperty('portion');
		expect(item).not.toHaveProperty('quantity');
		expect(item).not.toHaveProperty('amount_unit');
	});

	it('a saved meal without portion metadata falls back to grams', () => {
		const meal = {
			id: '0190e000-0000-7000-8000-000000000001',
			date: '2026-10-27',
			meal_type: 'lunch',
			description: 'shake',
			scale: 1,
			assumptions: null,
			confidence: null,
			source: 'manual',
			provider_id: null,
			updated_at: 0,
			items: [
				{
					id: '0190e000-0000-7000-8000-000000000002',
					name: 'Choc protein',
					portion_text: '1 serving (31 g)',
					grams: 31,
					kcal: 120,
					protein_g: 25,
					carbs_g: 2,
					fat_g: 1.5,
					food_id: 42,
					origin: 'food_db' as const
				}
			],
			totals: { kcal: 120, protein_g: 25, carbs_g: 2, fat_g: 1.5 }
		} satisfies Meal;
		const i = draftFromMeal(meal).items[0];
		expect(i.portions).toBeUndefined();
		expect(i.portion).toBeUndefined();
		expect(i.grams).toBe(31);
	});
});
