// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Meal, SavedProduct } from '#lib/api.ts';
import {
	blankItem,
	draftItemFromProduct,
	itemSourceLabel,
	mealSourceLabel,
	type DraftItem,
	useProduct,
	withGrams,
	withGramsInput,
	withKcal,
	withQuantity,
	withValue
} from './meal';

describe('source labels', () => {
	const item = (origin: Meal['items'][number]['origin'], food_id: number | null = null) =>
		({ origin, food_id }) as Meal['items'][number];
	const meal = (source: Meal['source'], items: Meal['items']) => ({ source, items }) as Meal;

	it('calls a manual meal made only of database foods a product', () => {
		expect(mealSourceLabel(meal('manual', [item('food_db', 7)]))).toBe('Product');
		expect(mealSourceLabel(meal('manual', [item('ai_estimate')]))).toBe('By hand');
		expect(mealSourceLabel(meal('manual', []))).toBe('By hand');
		expect(mealSourceLabel(meal('ai', [item('food_db', 7)]))).toBe('AI estimate');
		expect(mealSourceLabel(meal('repeat', [item('food_db', 7)]))).toBe('Eaten again');
	});

	it('labels items by where their numbers came from', () => {
		expect(itemSourceLabel(item('food_db', 7), meal('manual', []))).toEqual({ text: 'Database', kind: 'db' });
		expect(itemSourceLabel(item('ai_estimate'), meal('ai', []))).toEqual({ text: 'AI estimate', kind: 'ai' });
		expect(itemSourceLabel(item('ai_estimate'), meal('manual', []))).toEqual({ text: 'By hand', kind: 'plain' });
		// Direct edits: a database food whose numbers were changed, or an item typed in.
		expect(itemSourceLabel(item('manual', 7), meal('ai', []))).toEqual({ text: 'Edited', kind: 'plain' });
		expect(itemSourceLabel(item('manual'), meal('ai', []))).toEqual({ text: 'By hand', kind: 'plain' });
		expect(itemSourceLabel(item('manual'), meal('repeat', []))).toEqual({ text: 'By hand', kind: 'plain' });
		// A repeat or import can't tell a model estimate from a hand entry, so it claims neither.
		expect(itemSourceLabel(item('ai_estimate'), meal('repeat', []))).toBeNull();
		expect(itemSourceLabel(item('ai_estimate'), meal('import', []))).toBeNull();
	});
});

describe('editing numbers', () => {
	const base = (): DraftItem => ({
		...blankItem(),
		name: 'Whey',
		grams: 30,
		kcal: 120,
		protein_g: 24,
		carbs_g: 3,
		fat_g: 1.5,
		food_id: 9,
		origin: 'food_db'
	});

	it('withKcal scales the macros by the same ratio and keeps grams and food_id', () => {
		const i = withKcal(base(), 60);
		expect(i).toMatchObject({ kcal: 60, protein_g: 12, carbs_g: 1.5, fat_g: 0.75, grams: 30, food_id: 9, origin: 'manual' });
	});
	it('withKcal sets only kcal when there was none to scale from', () => {
		expect(withKcal({ ...base(), kcal: 0 }, 50)).toMatchObject({ kcal: 50, protein_g: 24, origin: 'manual' });
		expect(withKcal({ ...base(), kcal: null }, 50)).toMatchObject({ kcal: 50, protein_g: 24 });
		expect(withKcal(base(), null)).toMatchObject({ kcal: null, protein_g: 24 });
	});
	it('withValue is independent and marks the item manual', () => {
		expect(withValue(base(), 'protein_g', 10)).toMatchObject({ kcal: 120, protein_g: 10, origin: 'manual', food_id: 9 });
	});
	it('withGrams keeps the origin', () => {
		expect(withGrams(base(), 60)).toMatchObject({ kcal: 240, origin: 'food_db' });
	});
	it('blank items are manual', () => {
		expect(blankItem().origin).toBe('manual');
	});
});

describe('useProduct', () => {
	const powder = {
		food_id: 42,
		name: 'Choc protein',
		serving_g: 31,
		kcal: 387.1,
		protein_g: 80,
		carbs_g: 6.5,
		fat_g: 4.8
	} as SavedProduct;
	const item = (portion_text: string, grams: number | null): DraftItem => ({
		...blankItem(),
		name: 'chocolate protein powder',
		phrase: 'chocolate protein powder',
		portion_text,
		grams,
		kcal: 50,
		protein_g: 1,
		carbs_g: 1,
		fat_g: 1
	});

	it('uses count x serving when the portion is a count of a non-weight unit', () => {
		const i = useProduct(item('2 scoops', 20), powder);
		expect(i).toMatchObject({ name: 'Choc protein', food_id: 42, origin: 'food_db', grams: 62, swapped: true, remember: false });
		expect(i.kcal).toBeCloseTo(240, 0);
		expect(i.phrase).toBe('chocolate protein powder');
		expect(useProduct(item('a scoop', 20), powder).grams).toBe(31);
		expect(useProduct(item('1/2 scoop', 20), powder).grams).toBe(15.5);
	});
	it('keeps the grams for weight units, no serving, or no count', () => {
		expect(useProduct(item('40 g', 40), powder).grams).toBe(40);
		expect(useProduct(item('2 oz', 57), powder).grams).toBe(57);
		expect(useProduct(item('some', 25), powder).grams).toBe(25);
		expect(useProduct(item('2 scoops', 20), { ...powder, serving_g: null }).grams).toBe(20);
	});

	it('keeps a unit the product has and falls back to the best portion otherwise', () => {
		const withScoop = {
			...powder,
			portions: [
				{ unit: 'cup', grams: 80 },
				{ unit: 'serving', grams: 31 },
				{ unit: 'scoop', grams: 31 }
			]
		};
		expect(useProduct(item('2 scoops', 20), withScoop)).toMatchObject({
			portion: { unit: 'scoop', grams: 31 },
			quantity: 2,
			grams: 62,
			portion_text: '2 scoops (62 g)'
		});
		// "2 cups" means two of the product's own cups (80 g each), not two servings.
		expect(useProduct(item('2 cups', 20), withScoop)).toMatchObject({
			portion: { unit: 'cup', grams: 80 },
			quantity: 2,
			grams: 160,
			portion_text: '2 cups (160 g)'
		});
	});
});

describe('amount inputs', () => {
	const powder = {
		food_id: 42,
		name: 'Choc protein',
		serving_g: 31,
		kcal: 387.1,
		protein_g: 80,
		carbs_g: 6.5,
		fat_g: 4.8,
		portions: [
			{ unit: 'cup', grams: 80 },
			{ unit: 'serving', grams: 31 }
		]
	} as SavedProduct;
	const row = () => draftItemFromProduct(powder);

	it('scales grams and numbers from a typed quantity', () => {
		const next = withQuantity(row(), 2);
		expect(next.grams).toBe(160);
		expect(next.portion_text).toBe('2 cups (160 g)');
		expect(next.kcal).toBeCloseTo(619.4, 1);
	});

	it('leaves the row alone for a blank or zero quantity, so retyping keeps the numbers', () => {
		const before = row();
		expect(withQuantity(before, null)).toBe(before);
		expect(withQuantity(before, 0)).toBe(before);
		expect(withQuantity(before, Number.NaN)).toBe(before);
		// The old bug: clearing the field zeroed grams, and the next entry scaled from zero.
		const typed = withQuantity(before, 2);
		expect(typed.kcal).toBeCloseTo(before.kcal! * 2, 1);
		expect(typed.protein_g).toBeCloseTo(before.protein_g! * 2, 1);
	});

	it('clears grams on a manual row but keeps the amount on a product row', () => {
		expect(withGramsInput({ ...row(), editable: true }, null).grams).toBeNull();
		const product = row();
		expect(withGramsInput(product, null)).toBe(product);
		expect(withGramsInput(product, 0)).toBe(product);
	});

	it('reads grams instead of zero cups when the count rounds away', () => {
		const bigCup = { ...powder, portions: [{ unit: 'cup', grams: 240 }] } as SavedProduct;
		const next = withGramsInput(draftItemFromProduct(bigCup), 1);
		expect(next.grams).toBe(1);
		expect(next.quantity).toBe(0);
		expect(next.portion_text).toBe('1 g');
	});

	it('scales numbers down with the grams and labels the exact count', () => {
		const next = withGramsInput(row(), 40);
		expect(next.portion_text).toBe('½ cup (40 g)');
		expect(next.kcal).toBeCloseTo(387.1 * 0.4, 1);
	});
});
