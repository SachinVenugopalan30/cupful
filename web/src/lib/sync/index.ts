// SPDX-License-Identifier: AGPL-3.0-or-later
// The write contract (PLAN.md §6). Every write of logged data (meals, days) goes through
// `sendOps`. M6 puts an IndexedDB queue in front of this exact function, so never post to
// /api/sync any other way.

import { api, type Confidence, type MealItem, type MealSource, type MealType } from '../api';

// ---- UUIDv7 (RFC 9562): 48-bit ms timestamp, version 7, 12-bit counter, variant, 62 random bits ----

let lastMs = 0;
let seq = 0;

/**
 * A UUIDv7. Ids made by this page sort in creation order: within one millisecond (or if the clock
 * steps back) the 12-bit `rand_a` field counts up, borrowing the next millisecond on overflow.
 */
export function uuidv7(now = Date.now()): string {
	if (now > lastMs) {
		lastMs = now;
		seq = Math.floor(Math.random() * 0x800); // start low, leave room to count up
	} else if (++seq > 0xfff) {
		lastMs++;
		seq = 0;
	}
	const b = crypto.getRandomValues(new Uint8Array(16));
	for (let i = 0; i < 6; i++) b[i] = Math.floor(lastMs / 2 ** (8 * (5 - i))) & 0xff;
	b[6] = 0x70 | (seq >> 8);
	b[7] = seq & 0xff;
	b[8] = 0x80 | (b[8] & 0x3f);
	const h = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
	return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

// ---- ops ----

/** The full meal as `meal.upsert` sends it. Item values already include `scale`. */
export type MealIn = {
	id: string;
	date: string;
	meal_type: MealType;
	description: string;
	/** Display only ("×1.5"); never applied again. Use `scaleItems` before saving. */
	scale: number;
	assumptions: string | null;
	confidence: Confidence | null;
	source: MealSource;
	provider_id: number | null;
	items: MealItem[];
};

/** `null` clears a field; leave a field out to keep it. */
export type DayFields = {
	steps?: number | null;
	gym_minutes?: number | null;
	trained?: string[] | null;
	weight_kg?: number | null;
};

export type Op =
	| { op_id: string; kind: 'meal.upsert'; at: string; payload: MealIn }
	| { op_id: string; kind: 'meal.delete'; at: string; payload: { id: string } }
	| { op_id: string; kind: 'day.patch'; at: string; payload: DayFields & { date: string } };

export type OpResult = {
	op_id: string;
	status: 'applied' | 'duplicate' | 'stale' | 'rejected';
	/** Set when rejected. */
	error?: string;
};

const stamp = () => ({ op_id: uuidv7(), at: new Date().toISOString() });

export const mealUpsert = (meal: MealIn): Op => ({ ...stamp(), kind: 'meal.upsert', payload: meal });

export const mealDelete = (id: string): Op => ({ ...stamp(), kind: 'meal.delete', payload: { id } });

export function dayPatch(date: string, fields: DayFields): Op {
	const payload: DayFields & { date: string } = { date };
	for (const [k, v] of Object.entries(fields)) {
		if (v !== undefined) (payload as Record<string, unknown>)[k] = v;
	}
	return { ...stamp(), kind: 'day.patch', payload };
}

/** POSTs ops to /api/sync; one result per op, in order. Throws ApiError if the request fails. */
export async function sendOps(ops: Op[]): Promise<OpResult[]> {
	return (await api.post<{ results: OpResult[] }>('/api/sync', { ops })).results;
}

// ---- item helpers ----

/** Items with grams and nutrients multiplied by `k` (the portion multiplier), applied before saving. */
export function scaleItems(items: MealItem[], k: number): MealItem[] {
	return items.map((i) => ({
		...i,
		grams: i.grams === null ? null : i.grams * k,
		kcal: i.kcal * k,
		protein_g: i.protein_g * k,
		carbs_g: i.carbs_g * k,
		fat_g: i.fat_g * k
	}));
}

type Per100 = { food_id: number; name: string; kcal: number; protein_g: number; carbs_g: number; fat_g: number };

/** A `food_db` item for `grams` of a food given per 100 g (a product, say). */
export function itemFromPer100(food: Per100, grams: number, portion_text: string): MealItem {
	return {
		id: uuidv7(),
		name: food.name,
		portion_text,
		grams,
		kcal: (food.kcal * grams) / 100,
		protein_g: (food.protein_g * grams) / 100,
		carbs_g: (food.carbs_g * grams) / 100,
		fat_g: (food.fat_g * grams) / 100,
		food_id: food.food_id,
		origin: 'food_db'
	};
}
