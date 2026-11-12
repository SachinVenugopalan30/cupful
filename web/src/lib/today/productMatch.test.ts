// SPDX-License-Identifier: AGPL-3.0-or-later
// Exact saved-product matches on AI estimate items: normalization, no fuzzy names, and no flag
// on items that are already a product or were swapped by hand.
import { describe, expect, it } from 'vitest';
import type { SavedProduct } from '#lib/api.ts';
import { blankItem, type DraftItem } from './meal';
import { matchProduct, normalizeName } from './productMatch';

const saved = (food_id: number, name: string): SavedProduct =>
	({
		food_id,
		name,
		brand: 'Acme',
		barcode: null,
		source: 'user',
		serving_g: 30,
		serving_unit: 'g',
		serving_text: null,
		attribution: null,
		kcal: 380,
		protein_g: 80,
		carbs_g: 5,
		fat_g: 4,
		last_servings: 1,
		last_used_at: 0
	}) as SavedProduct;

const ai = (name: string): DraftItem => ({ ...blankItem(), name, phrase: name, origin: 'ai_estimate' });

describe('normalizeName', () => {
	it('trims, lowercases and collapses whitespace', () => {
		expect(normalizeName('  Chocolate \n\t Protein   Powder ')).toBe('chocolate protein powder');
	});
});

describe('matchProduct', () => {
	const products = [saved(1, 'Chocolate protein powder'), saved(2, 'Oat milk')];

	it('matches an exact normalized name, ignoring case and spacing', () => {
		expect(matchProduct(ai('chocolate protein powder'), products)?.food_id).toBe(1);
		expect(matchProduct(ai('  CHOCOLATE   Protein  powder '), products)?.food_id).toBe(1);
	});

	it('picks the right product from several saved products', () => {
		expect(matchProduct(ai('Oat milk'), products)?.food_id).toBe(2);
	});

	it('does not match a partial name or a brand', () => {
		expect(matchProduct(ai('chocolate protein'), products)).toBeNull();
		expect(matchProduct(ai('Acme chocolate protein powder'), products)).toBeNull();
	});

	it('does not flag manual items', () => {
		expect(matchProduct({ ...ai('Oat milk'), origin: 'manual' }, products)).toBeNull();
	});

	it('does not flag an item already using the product', () => {
		const item = { ...ai('Oat milk'), origin: 'food_db' as const, food_id: 2 };
		expect(matchProduct(item, products)).toBeNull();
	});

	it('does not flag a swapped item', () => {
		expect(matchProduct({ ...ai('Oat milk'), swapped: true }, products)).toBeNull();
	});

	it('does not flag food_db items that point at another food', () => {
		expect(matchProduct({ ...ai('Oat milk'), origin: 'food_db', food_id: 9 }, products)).toBeNull();
	});

	it('returns null while the product list is loading, failed, or empty', () => {
		expect(matchProduct(ai('Oat milk'), null)).toBeNull();
		expect(matchProduct(ai('Oat milk'), [])).toBeNull();
		expect(matchProduct({ ...ai('   ') }, products)).toBeNull();
	});
});
