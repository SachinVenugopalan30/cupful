// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	dayPatch,
	itemFromPer100,
	mealDelete,
	mealUpsert,
	scaleItems,
	sendOps,
	uuidv7,
	type MealIn
} from './index';

const UUIDV7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe('uuidv7', () => {
	it('has the v7 layout and variant', () => {
		for (let i = 0; i < 100; i++) expect(uuidv7()).toMatch(UUIDV7);
	});

	it('carries the timestamp in the first 48 bits', () => {
		const ms = Date.UTC(2030, 0, 1, 12, 0, 0, 123); // later than any id made so far
		const hex = uuidv7(ms).replace(/-/g, '').slice(0, 12);
		expect(parseInt(hex, 16)).toBe(ms);
	});

	it('sorts in creation order, also within one millisecond', () => {
		const ms = Date.now();
		const ids = Array.from({ length: 5000 }, () => uuidv7(ms));
		expect([...ids].sort()).toEqual(ids);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('never goes backwards when the clock does', () => {
		const a = uuidv7(2_000_000_000_000);
		const b = uuidv7(1_000_000_000_000);
		expect(b > a).toBe(true);
	});
});

const meal: MealIn = {
	id: uuidv7(),
	date: '2026-10-28',
	meal_type: 'lunch',
	description: 'rice and dal',
	scale: 1,
	assumptions: null,
	confidence: null,
	source: 'manual',
	provider_id: null,
	items: [
		{
			id: uuidv7(),
			name: 'rice',
			portion_text: '1 cup',
			grams: 160,
			kcal: 208,
			protein_g: 4.3,
			carbs_g: 45,
			fat_g: 0.5,
			food_id: 1,
			origin: 'food_db'
		}
	]
};

describe('op builders', () => {
	it('meal.upsert carries the full meal', () => {
		const op = mealUpsert(meal);
		expect(op.kind).toBe('meal.upsert');
		expect(op.op_id).toMatch(UUIDV7);
		expect(op.payload).toEqual(meal);
		expect(new Date(op.at).toISOString()).toBe(op.at);
	});

	it('meal.delete carries only the id', () => {
		expect(mealDelete('abc').payload).toEqual({ id: 'abc' });
		expect(mealDelete('abc').kind).toBe('meal.delete');
	});

	it('day.patch keeps null (clear) and drops undefined fields', () => {
		const op = dayPatch('2026-10-28', { weight_kg: 81.4, steps: null, gym_minutes: undefined });
		expect(op.kind).toBe('day.patch');
		expect(op.payload).toEqual({ date: '2026-10-28', weight_kg: 81.4, steps: null });
		expect('gym_minutes' in op.payload).toBe(false);
	});
});

describe('scaleItems', () => {
	it('multiplies grams and nutrients, keeping ids', () => {
		const [s] = scaleItems(meal.items, 1.5);
		expect(s).toMatchObject({
			id: meal.items[0].id,
			grams: 240,
			kcal: 312,
			protein_g: 4.3 * 1.5,
			carbs_g: 67.5,
			fat_g: 0.75
		});
	});

	it('leaves missing grams missing', () => {
		const [s] = scaleItems([{ ...meal.items[0], grams: null }], 2);
		expect(s.grams).toBeNull();
		expect(s.kcal).toBe(416);
	});

	it('does not mutate its input', () => {
		scaleItems(meal.items, 3);
		expect(meal.items[0].kcal).toBe(208);
	});
});

describe('itemFromPer100', () => {
	it('computes an item from per-100 g values', () => {
		const item = itemFromPer100(
			{ food_id: 7, name: 'Biscuits', kcal: 480, protein_g: 6, carbs_g: 70, fat_g: 20 },
			30,
			'1.5 servings'
		);
		expect(item).toMatchObject({
			name: 'Biscuits',
			portion_text: '1.5 servings',
			grams: 30,
			kcal: 144,
			protein_g: 1.8,
			carbs_g: 21,
			fat_g: 6,
			food_id: 7,
			origin: 'food_db'
		});
		expect(item.id).toMatch(UUIDV7);
	});
});

describe('sendOps', () => {
	afterEach(() => vi.unstubAllGlobals());

	it('posts the ops with the CSRF header and returns per-op results', async () => {
		const results = [{ op_id: 'x', status: 'applied' }];
		const fetch = vi.fn(async () => new Response(JSON.stringify({ results }), { status: 200 }));
		vi.stubGlobal('fetch', fetch);
		const op = mealDelete('m1');
		expect(await sendOps([op])).toEqual(results);
		const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/sync');
		expect(init.method).toBe('POST');
		expect(new Headers(init.headers).get('X-Cupful')).toBe('1');
		expect(JSON.parse(init.body as string)).toEqual({ ops: [op] });
	});
});
