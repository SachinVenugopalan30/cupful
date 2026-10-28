// SPDX-License-Identifier: AGPL-3.0-or-later
// Meal-flow logic for the composer, review and meal list: drafts, item edits, totals, payloads.

import type {
	Confidence,
	Estimate,
	EstimateItem,
	Macros,
	Meal,
	MealItem,
	MealSource,
	MealType,
	Origin
} from '#lib/api.ts';
import { scaleItems, uuidv7, type MealIn } from '#lib/sync/index.ts';

export const MEAL_TYPES: MealType[] = ['breakfast', 'lunch', 'dinner', 'snack'];
export const MULTIPLIERS = [0.5, 0.75, 1, 1.25, 1.5, 2];
export const MAX_DESCRIPTION = 1000;

/** Photo size after downscaling the long side to `max` (never upscaled). */
export function fitWithin(width: number, height: number, max: number) {
	const k = Math.min(1, max / Math.max(width, height));
	return { width: Math.max(1, Math.round(width * k)), height: Math.max(1, Math.round(height * k)) };
}

type Nums = { kcal: number | null; protein_g: number | null; carbs_g: number | null; fat_g: number | null };
export type NumKey = keyof Nums;
const KEYS: NumKey[] = ['kcal', 'protein_g', 'carbs_g', 'fat_g'];

/** One choice for an ambiguous item: the model's estimate or a database candidate, at `grams`. */
type Option = Nums & { name: string; food_id: number | null; origin: Origin; grams: number };

export type DraftItem = Nums & {
	id: string;
	name: string;
	portion_text: string;
	grams: number | null;
	food_id: number | null;
	origin: Origin;
	/** Show inputs for name and numbers (manual items, items the pipeline couldn't fill). */
	editable: boolean;
	/** Ambiguity choices; empty when there's nothing to choose. */
	options: Option[];
	chosen: number;
};

export type Draft = {
	id: string;
	date: string;
	meal_type: MealType;
	description: string;
	scale: number;
	assumptions: string | null;
	confidence: Confidence | null;
	source: MealSource;
	provider_id: number | null;
	warnings: string[];
	items: DraftItem[];
};

const pick = (n: Nums): Nums => ({ kcal: n.kcal, protein_g: n.protein_g, carbs_g: n.carbs_g, fat_g: n.fat_g });
const scaleNums = (n: Nums, k: number): Nums =>
	Object.fromEntries(KEYS.map((key) => [key, n[key] === null ? null : n[key]! * k])) as Nums;

function fromEstimateItem(e: EstimateItem): DraftItem {
	const options: Option[] =
		e.match === 'ambiguous'
			? [
					...(e.origin === 'ai_estimate'
						? [{ ...pick(e), name: e.name, food_id: null, origin: e.origin, grams: e.grams }]
						: []),
					...e.candidates.map((c) => ({
						...pick(c),
						name: c.name,
						food_id: c.food_id,
						origin: 'food_db' as const,
						grams: e.grams
					}))
				]
			: [];
	return {
		id: uuidv7(),
		name: e.name,
		portion_text: e.portion_text,
		grams: e.grams,
		...pick(e),
		food_id: e.food_id,
		origin: e.origin,
		editable: KEYS.some((k) => e[k] === null),
		options,
		chosen: Math.max(0, options.findIndex((o) => o.food_id === e.food_id))
	};
}

export function blankItem(): DraftItem {
	return {
		id: uuidv7(),
		name: '',
		portion_text: '',
		grams: null,
		kcal: null,
		protein_g: null,
		carbs_g: null,
		fat_g: null,
		food_id: null,
		origin: 'ai_estimate', // the server's only non-database origin; the meal's source says "manual"
		editable: true,
		options: [],
		chosen: 0
	};
}

/** Every number filled in and a name: ready to save. */
export const itemReady = (i: DraftItem) => i.name.trim() !== '' && KEYS.every((k) => i[k] !== null && i[k]! >= 0);

/** New grams; numbers follow proportionally (per-gram values stay the same). */
export function withGrams(i: DraftItem, grams: number): DraftItem {
	const k = i.grams && i.grams > 0 ? grams / i.grams : 1;
	return { ...i, grams, ...(i.grams && i.grams > 0 ? scaleNums(i, k) : {}) };
}

export const withValue = (i: DraftItem, key: NumKey, v: number | null): DraftItem => ({ ...i, [key]: v });

/** Swap in ambiguity option `n`, scaled to the item's current grams. */
export function choose(i: DraftItem, n: number): DraftItem {
	const o = i.options[n];
	const k = i.grams && o.grams > 0 ? i.grams / o.grams : 1;
	return { ...i, chosen: n, name: o.name, food_id: o.food_id, origin: o.origin, ...scaleNums(o, k) };
}

export function draftFromEstimate(
	e: Estimate,
	ctx: { date: string; meal_type: MealType; description: string }
): Draft {
	return {
		id: uuidv7(),
		...ctx,
		scale: 1,
		assumptions: e.assumptions,
		confidence: e.confidence,
		source: 'ai',
		provider_id: e.provider_id,
		warnings: e.warnings,
		items: e.items.map(fromEstimateItem)
	};
}

export function manualDraft(ctx: { date: string; meal_type: MealType; description: string }): Draft {
	return {
		id: uuidv7(),
		...ctx,
		scale: 1,
		assumptions: null,
		confidence: null,
		source: 'manual',
		provider_id: null,
		warnings: [],
		items: [blankItem()]
	};
}

/** Reopen a saved meal for editing: same ids; stored items include `scale`, so undo it here. */
export function draftFromMeal(m: Meal): Draft {
	return {
		id: m.id,
		date: m.date,
		meal_type: m.meal_type,
		description: m.description,
		scale: m.scale,
		assumptions: m.assumptions,
		confidence: m.confidence,
		source: m.source,
		provider_id: m.provider_id,
		warnings: [],
		items: scaleItems(m.items, 1 / m.scale).map((i) => ({ ...i, editable: false, options: [], chosen: 0 }))
	};
}

/** The full `meal.upsert` payload, with the multiplier applied to items. */
export function mealFromDraft(d: Draft): MealIn {
	if (!d.items.every(itemReady)) throw new Error('An item needs a name and numbers.');
	const items: MealItem[] = d.items.map((i) => ({
		id: i.id,
		name: i.name.trim(),
		portion_text: i.portion_text,
		grams: i.grams,
		kcal: i.kcal!,
		protein_g: i.protein_g!,
		carbs_g: i.carbs_g!,
		fat_g: i.fat_g!,
		food_id: i.food_id,
		origin: i.origin
	}));
	const { warnings: _w, items: _i, ...meal } = d;
	return { ...meal, items: scaleItems(items, d.scale) };
}

/** "Eat this again": the full meal on `date`, new ids, no model call. Items already include scale. */
export function repeatMeal(m: Meal, date: string): MealIn {
	return {
		id: uuidv7(),
		date,
		meal_type: m.meal_type,
		description: m.description,
		scale: m.scale,
		assumptions: m.assumptions,
		confidence: m.confidence,
		source: 'repeat',
		provider_id: null,
		items: m.items.map((i) => ({ ...i, id: uuidv7() }))
	};
}

const zero = (): Macros => ({ kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 });

/** Draft totals times the multiplier; missing numbers count as 0. */
export function sumItems(items: Nums[], scale: number): Macros {
	const t = zero();
	for (const i of items) for (const k of KEYS) t[k] += (i[k] ?? 0) * scale;
	return t;
}

export function dayTotals(meals: Meal[]): Macros {
	return sumItems(meals.map((m) => m.totals), 1);
}

export const PHOTO_MAX_SIDE = 1600;

/**
 * Re-encodes a photo as JPEG through a canvas, downscaled to PHOTO_MAX_SIDE. Drawing to a canvas
 * keeps only pixels, so EXIF (GPS location included) is gone; orientation is applied first.
 */
export async function reencodePhoto(file: Blob): Promise<Blob> {
	const bmp = await createImageBitmap(file, { imageOrientation: 'from-image' });
	const { width, height } = fitWithin(bmp.width, bmp.height, PHOTO_MAX_SIDE);
	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	canvas.getContext('2d')!.drawImage(bmp, 0, 0, width, height);
	bmp.close();
	return new Promise((resolve, reject) =>
		canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("Couldn't read this photo."))), 'image/jpeg', 0.85)
	);
}
