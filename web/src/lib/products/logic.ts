// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure logic for packaged products: barcodes, servings ↔ grams, label checks, the logged meal.

import type { MealType, Product } from '../api';
import { formatGrams, round } from '../format';
import { itemFromPer100, uuidv7, type MealIn } from '../sync';

/** 8–14 digits whose last digit is the GTIN check digit (same rule as server `products::gtin_ok`). */
export function gtinOk(code: string): boolean {
	if (!/^\d{8,14}$/.test(code)) return false;
	let sum = 0;
	for (let i = 0; i < code.length; i++) {
		sum += Number(code[code.length - 1 - i]) * (i % 2 ? 3 : 1);
	}
	return sum % 10 === 0;
}

/** UPC-E (8 digits) to its UPC-A; the check digit is the same. */
function expandUpcE(e: string): string {
	const [ns, x1, x2, x3, x4, x5, x6, check] = e;
	const body =
		x6 <= '2'
			? `${x1}${x2}${x6}0000${x3}${x4}${x5}`
			: x6 === '3'
				? `${x1}${x2}${x3}00000${x4}${x5}`
				: x6 === '4'
					? `${x1}${x2}${x3}${x4}00000${x5}`
					: `${x1}${x2}${x3}${x4}${x5}0000${x6}`;
	return ns + body + check;
}

/** A scanned or typed code ready for lookup, or null if it isn't a valid GTIN. */
export function normalizeScan(raw: string, format = ''): string | null {
	let code = raw.trim();
	if (/upc[-_]?e/i.test(format) && /^\d{8}$/.test(code)) code = expandUpcE(code);
	return gtinOk(code) ? code : null;
}


export const servingsToGrams = (servings: number, servingG: number) => round(servings * servingG, 1);
export const gramsToServings = (grams: number, servingG: number) => round(grams / servingG, 2);

/** One 0.5 step up or down from `s`, snapping to the 0.5 grid; at least 0.5. */
export function stepServings(s: number, dir: 1 | -1): number {
	const next = dir > 0 ? Math.floor(s * 2 + 1e-9) / 2 + 0.5 : Math.ceil(s * 2 - 1e-9) / 2 - 0.5;
	return Math.max(0.5, next);
}

/** "1.5 servings (45 g)", "1 serving (30 g)", or "45 g" without a serving size. */
export function portionText(grams: number, servingG: number | null): string {
	const g = `${formatGrams(grams)} g`;
	if (!servingG) return g;
	const s = gramsToServings(grams, servingG);
	return `${s} serving${s === 1 ? '' : 's'} (${g})`;
}

/** The meal Add logs: one `food_db` item for `grams` of the product. */
export function buildMeal(p: Product, grams: number, date: string, mealType: MealType): MealIn {
	return {
		id: uuidv7(),
		date,
		meal_type: mealType,
		description: p.name,
		scale: 1,
		assumptions: null,
		confidence: null,
		source: 'manual',
		provider_id: null,
		items: [itemFromPer100(p, grams, portionText(grams, p.serving_g))]
	};
}

export function defaultMealType(hour: number): MealType {
	if (hour >= 5 && hour < 11) return 'breakfast';
	if (hour >= 11 && hour < 15) return 'lunch';
	if (hour >= 18 && hour < 22) return 'dinner';
	return 'snack';
}

// ---- "Enter from the label" ----

export type LabelForm = {
	name: string;
	brand: string;
	barcode: string;
	basis: '100g' | 'serving';
	serving_g: number | null;
	kcal: number;
	protein_g: number;
	carbs_g: number;
	fat_g: number;
};

const MACROS = ['kcal', 'protein_g', 'carbs_g', 'fat_g'] as const;

export function labelToPer100(f: LabelForm) {
	const k = f.basis === 'serving' && f.serving_g ? 100 / f.serving_g : 1;
	return Object.fromEntries(MACROS.map((m) => [m, round(f[m] * k, 6)])) as Record<
		(typeof MACROS)[number],
		number
	>;
}

/** The first problem with a label entry, by field, with the server's limits; null if fine. */
export function labelProblem(f: LabelForm): { field: keyof LabelForm; message: string } | null {
	const name = f.name.trim();
	if (!name || name.length > 200) return { field: 'name', message: 'Enter a name (up to 200 characters).' };
	if (f.brand.trim().length > 200) return { field: 'brand', message: 'Brand is longer than 200 characters.' };
	if (!gtinOk(f.barcode)) return { field: 'barcode', message: 'Barcode must be 8–14 digits with a valid check digit.' };
	if (f.basis === 'serving' && !(f.serving_g && f.serving_g > 0 && f.serving_g <= 2000)) {
		return { field: 'serving_g', message: 'Enter the serving size in grams (up to 2000).' };
	}
	const per100 = labelToPer100(f);
	const unit = f.basis === 'serving' ? 'per serving' : 'per 100 g';
	if (!(per100.kcal >= 0 && per100.kcal <= 900)) {
		return { field: 'kcal', message: `Check the kcal ${unit}: that's not 0–900 kcal per 100 g.` };
	}
	for (const m of ['protein_g', 'carbs_g', 'fat_g'] as const) {
		if (!(per100[m] >= 0 && per100[m] <= 100)) {
			return { field: m, message: `Check this value ${unit}: macros must be 0–100 g per 100 g.` };
		}
	}
	return null;
}

// ---- saved products ----

export function filterSaved<T extends { name: string; brand: string | null }>(list: T[], q: string): T[] {
	const needle = q.trim().toLowerCase();
	if (!needle) return list;
	return list.filter((p) => `${p.name} ${p.brand ?? ''}`.toLowerCase().includes(needle));
}

// ---- scanning ----

export const NATIVE_FORMATS = ['ean_13', 'ean_8', 'upc_a', 'upc_e'];

/** `native` when the browser's BarcodeDetector reads EAN-13; otherwise the WASM reader. */
export async function pickDecoder(g: {
	BarcodeDetector?: { getSupportedFormats(): Promise<string[]> };
}): Promise<'native' | 'wasm'> {
	try {
		if (g.BarcodeDetector && (await g.BarcodeDetector.getSupportedFormats()).includes('ean_13')) {
			return 'native';
		}
	} catch {
		// fall through to WASM
	}
	return 'wasm';
}

/** Plain words for why the camera can't be used, or null. */
export function cameraProblem(s: { isSecureContext?: boolean; hasMediaDevices?: boolean; error?: unknown }): string | null {
	if (s.isSecureContext === false) {
		return 'The camera needs a secure connection (HTTPS, or localhost). Open Cupful over HTTPS, or type the barcode below.';
	}
	if (s.hasMediaDevices === false) return "This browser can't use the camera here. Type the barcode below.";
	if (s.error) {
		const name = (s.error as { name?: string }).name;
		if (name === 'NotAllowedError' || name === 'SecurityError') {
			return 'Camera permission was denied. Allow camera access in your browser settings, or type the barcode below.';
		}
		if (name === 'NotFoundError' || name === 'OverconstrainedError') {
			return 'No camera found. Type the barcode below.';
		}
		if (name === 'NotReadableError') return 'The camera is in use by another app. Close it and try again, or type the barcode below.';
		return "The camera couldn't start. Type the barcode below.";
	}
	return null;
}
