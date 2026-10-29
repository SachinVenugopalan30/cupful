// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { AliasRow, Product } from '../api';
import {
	buildMeal,
	cameraProblem,
	defaultMealType,
	filterSaved,
	sortPhrases,
	gramsToServings,
	gtinOk,
	labelProblem,
	labelToPer100,
	normalizeScan,
	pickDecoder,
	portionText,
	servingsToGrams,
	stepServings
} from './logic';

const product: Product = {
	food_id: 7,
	source: 'user',
	barcode: '4006381333931',
	name: 'Oat biscuits',
	brand: 'Acme',
	serving_g: 30,
	serving_text: '2 biscuits',
	kcal: 480,
	protein_g: 8,
	carbs_g: 64,
	fat_g: 20,
	attribution: null
};

describe('gtinOk (mirrors server products::gtin_ok)', () => {
	it('accepts valid EAN-13, UPC-A, EAN-8, GTIN-14', () => {
		for (const c of ['4006381333931', '036000291452', '96385074', '00012345600012']) {
			expect(gtinOk(c)).toBe(true);
		}
	});
	it('rejects bad check digit, wrong length, non-digits', () => {
		for (const c of ['4006381333932', '1234565', '123456789012345', '40063813339a1', '+4006381333931', '']) {
			expect(gtinOk(c)).toBe(false);
		}
	});
});

describe('normalizeScan', () => {
	it('trims and checks', () => {
		expect(normalizeScan(' 4006381333931 ')).toBe('4006381333931');
		expect(normalizeScan('4006381333932')).toBeNull();
	});
	it('expands UPC-E to UPC-A (same check digit)', () => {
		// UPC-E 01234565 = UPC-A 012345000065
		expect(normalizeScan('01234565', 'upc_e')).toBe('012345000065');
		expect(normalizeScan('04252614', 'UPC-E')).toBe('042100005264');
	});
});

describe('servings and grams', () => {
	it('servings to grams, one decimal', () => {
		expect(servingsToGrams(1.5, 30)).toBe(45);
		expect(servingsToGrams(0.5, 33.3)).toBe(16.7);
	});
	it('grams to servings, two decimals', () => {
		expect(gramsToServings(45, 30)).toBe(1.5);
		expect(gramsToServings(40, 30)).toBe(1.33);
	});
	it('steps by 0.5, snapping to the grid, never below 0.5', () => {
		expect(stepServings(1, 1)).toBe(1.5);
		expect(stepServings(1.33, 1)).toBe(1.5);
		expect(stepServings(1.33, -1)).toBe(1);
		expect(stepServings(1.5, -1)).toBe(1);
		expect(stepServings(0.5, -1)).toBe(0.5);
		expect(stepServings(0.2, -1)).toBe(0.5);
	});
	it('portion text', () => {
		expect(portionText(45, 30)).toBe('1.5 servings (45 g)');
		expect(portionText(30, 30)).toBe('1 serving (30 g)');
		expect(portionText(125.5, null)).toBe('125.5 g');
	});
});

describe('buildMeal', () => {
	it('1.5 servings: one food_db item with values for 45 g', () => {
		const m = buildMeal(product, 45, '2026-10-28', 'snack');
		expect(m).toMatchObject({
			date: '2026-10-28',
			meal_type: 'snack',
			description: 'Oat biscuits',
			scale: 1,
			source: 'manual',
			assumptions: null,
			confidence: null,
			provider_id: null
		});
		expect(m.id).toMatch(/^[0-9a-f-]{36}$/);
		expect(m.items).toHaveLength(1);
		const i = m.items[0];
		expect(i).toMatchObject({
			name: 'Oat biscuits',
			food_id: 7,
			origin: 'food_db',
			grams: 45,
			portion_text: '1.5 servings (45 g)'
		});
		expect(i.kcal).toBeCloseTo(216);
		expect(i.protein_g).toBeCloseTo(3.6);
		expect(i.carbs_g).toBeCloseTo(28.8);
		expect(i.fat_g).toBeCloseTo(9);
	});
});

describe('label entry', () => {
	const label = {
		name: 'Crackers',
		brand: '',
		barcode: '4006381333931',
		basis: 'serving' as const,
		serving_g: 25,
		kcal: 120,
		protein_g: 2,
		carbs_g: 16,
		fat_g: 5
	};
	it('per serving to per 100 g', () => {
		expect(labelToPer100(label)).toEqual({ kcal: 480, protein_g: 8, carbs_g: 64, fat_g: 20 });
		expect(labelToPer100({ ...label, basis: '100g' })).toEqual({ kcal: 120, protein_g: 2, carbs_g: 16, fat_g: 5 });
	});
	it('valid label has no problem', () => {
		expect(labelProblem(label)).toBeNull();
	});
	it('validates like the server', () => {
		expect(labelProblem({ ...label, name: '  ' })?.field).toBe('name');
		expect(labelProblem({ ...label, name: 'x'.repeat(201) })?.field).toBe('name');
		expect(labelProblem({ ...label, barcode: '4006381333932' })?.field).toBe('barcode');
		expect(labelProblem({ ...label, serving_g: null })?.field).toBe('serving_g');
		expect(labelProblem({ ...label, serving_g: 2001 })?.field).toBe('serving_g');
		// 250 kcal per 25 g serving = 1000 per 100 g
		expect(labelProblem({ ...label, kcal: 250 })?.field).toBe('kcal');
		expect(labelProblem({ ...label, kcal: NaN })?.field).toBe('kcal');
		expect(labelProblem({ ...label, fat_g: 26 })?.field).toBe('fat_g');
		expect(labelProblem({ ...label, protein_g: -1 })?.field).toBe('protein_g');
		expect(labelProblem({ ...label, basis: '100g', serving_g: null, kcal: 900 })).toBeNull();
	});
});

describe('defaultMealType', () => {
	it('by hour', () => {
		expect(defaultMealType(7)).toBe('breakfast');
		expect(defaultMealType(13)).toBe('lunch');
		expect(defaultMealType(16)).toBe('snack');
		expect(defaultMealType(19)).toBe('dinner');
		expect(defaultMealType(23)).toBe('snack');
		expect(defaultMealType(2)).toBe('snack');
	});
});

describe('filterSaved', () => {
	const list = [
		{ ...product, name: 'Oat biscuits', brand: 'Acme' },
		{ ...product, food_id: 8, name: 'Greek yogurt', brand: 'Fage' }
	];
	it('matches name or brand, case-insensitive; empty query keeps all', () => {
		expect(filterSaved(list, 'fage').map((p) => p.food_id)).toEqual([8]);
		expect(filterSaved(list, 'OAT').map((p) => p.food_id)).toEqual([7]);
		expect(filterSaved(list, '  ')).toHaveLength(2);
	});
});

describe('pickDecoder', () => {
	const detector = (formats: string[]) =>
		class {
			static async getSupportedFormats() {
				return formats;
			}
		};
	it('native when BarcodeDetector supports EAN-13', async () => {
		expect(await pickDecoder({ BarcodeDetector: detector(['qr_code', 'ean_13', 'upc_a']) })).toBe('native');
	});
	it('wasm when BarcodeDetector is missing', async () => {
		expect(await pickDecoder({})).toBe('wasm');
	});
	it('wasm when BarcodeDetector lacks retail formats', async () => {
		expect(await pickDecoder({ BarcodeDetector: detector(['qr_code']) })).toBe('wasm');
	});
	it('wasm when getSupportedFormats throws', async () => {
		const Broken = class {
			static async getSupportedFormats(): Promise<string[]> {
				throw new Error('nope');
			}
		};
		expect(await pickDecoder({ BarcodeDetector: Broken })).toBe('wasm');
	});
});

describe('cameraProblem', () => {
	it('insecure context or no mediaDevices', () => {
		expect(cameraProblem({ isSecureContext: false, hasMediaDevices: true })).toMatch(/HTTPS/);
		expect(cameraProblem({ isSecureContext: true, hasMediaDevices: false })).toMatch(/camera/i);
	});
	it('getUserMedia errors', () => {
		const err = (name: string) => Object.assign(new Error(name), { name });
		expect(cameraProblem({ error: err('NotAllowedError') })).toMatch(/permission/i);
		expect(cameraProblem({ error: err('NotFoundError') })).toMatch(/No camera/);
		expect(cameraProblem({ error: err('NotReadableError') })).toMatch(/another app/);
	});
	it('fine otherwise', () => {
		expect(cameraProblem({ isSecureContext: true, hasMediaDevices: true })).toBeNull();
	});
});

describe('sortPhrases', () => {
	const row = (id: number, alias: string, food_id: number, food_name: string, barcode: string | null = '1'): AliasRow => ({
		id, alias, food_id, food_name, brand: null, barcode, source: 'user'
	});
	const rows = [
		row(1, 'oat milk', 5, 'Oat drink'),
		row(2, 'chocolate whey', 6, 'Other whey'),
		row(3, 'my whey', 7, 'Mine'),
		row(4, 'whey scoop', 7, 'Mine'),
		row(5, 'rice', 8, 'Staple rice', null),
		row(6, 'protein shake', 9, 'Shake')
	];
	it('splits phrases for this product from other products, sharing a word first', () => {
		const { mine, others } = sortPhrases(rows, 'Whey Protein Chocolate', 7);
		expect(mine.map((a) => a.id)).toEqual([3, 4]);
		// shares "whey" (2) and "protein" (6); others keep their order; database foods and short words don't count
		expect(others.map((a) => a.id)).toEqual([2, 6, 1]);
	});
});
