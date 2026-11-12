// SPDX-License-Identifier: AGPL-3.0-or-later
// Review builder helpers: a product as a draft item, appending without a blank starter row, and
// the meal name carried through saving.
import { describe, expect, it } from 'vitest';
import type { Product } from '#lib/api.ts';
import {
	appendProduct,
	blankItem,
	draftItemFromProduct,
	manualDraft,
	mealFromDraft
} from './meal';

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
