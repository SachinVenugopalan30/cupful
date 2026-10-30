// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Day, Meal, MealItem } from '../api.ts';
import type { Op } from '../sync/index.ts';
import { overlayDay, overlayMeals } from './overlay.ts';
import type { QueuedOp, QueueStatus } from './types.ts';

function item(id: string, kcal: number, protein = 10): MealItem {
	return {
		id,
		name: `Item ${id}`,
		portion_text: '1 serving',
		grams: 100,
		kcal,
		protein_g: protein,
		carbs_g: 20,
		fat_g: 5,
		food_id: null,
		origin: 'ai_estimate'
	};
}

function meal(id: string, items: MealItem[], date = '2026-10-29'): Meal {
	return {
		id,
		date,
		meal_type: 'lunch',
		description: 'Lunch',
		scale: 1,
		assumptions: null,
		confidence: 'high',
		source: 'ai',
		provider_id: null,
		updated_at: 1,
		items,
		totals: { kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 }
	};
}

function queued(op: Op, status: QueueStatus = 'pending'): QueuedOp {
	return {
		op_id: op.op_id,
		user_id: 1,
		op,
		status,
		queued_at: 1,
		attempts: 0
	};
}

function upsert(id: string, items: MealItem[], date = '2026-10-29'): Op {
	return {
		op_id: `op-upsert-${id}`,
		kind: 'meal.upsert',
		at: '2026-10-29T12:00:00.000Z',
		payload: {
			id,
			date,
			meal_type: 'lunch',
			description: 'Updated lunch',
			scale: 1,
			assumptions: null,
			confidence: null,
			source: 'manual',
			provider_id: null,
			items
		}
	};
}

function day(overrides: Partial<Day> = {}): Day {
	return {
		date: '2026-10-29',
		steps: 1000,
		steps_source: 'google',
		gym_minutes: null,
		gym_source: null,
		trained: ['Legs'],
		weight_kg: null,
		...overrides
	};
}

describe('overlayMeals', () => {
	it('adds a queued meal and computes totals from its items in order', () => {
		const result = overlayMeals([], [queued(upsert('m1', [item('i1', 300), item('i2', 200, 5)]))], '2026-10-29');
		expect(result).toHaveLength(1);
		expect(result[0].description).toBe('Updated lunch');
		expect(result[0].totals).toEqual({ kcal: 500, protein_g: 15, carbs_g: 40, fat_g: 10 });
	});

	it('replaces an existing meal and recomputes totals', () => {
		const base = [{ ...meal('m1', [item('i1', 1000)]), totals: { kcal: 1000, protein_g: 10, carbs_g: 20, fat_g: 5 } }];
		const result = overlayMeals(base, [queued(upsert('m1', [item('i1', 250)]))], '2026-10-29');
		expect(result).toHaveLength(1);
		expect(result[0].items.map((i) => i.id)).toEqual(['i1']);
		expect(result[0].totals.kcal).toBe(250);
	});

	it('removes a deleted meal', () => {
		const base = [meal('m1', [item('i1', 100)]), meal('m2', [item('i2', 200)])];
		const op: Op = {
			op_id: 'del-1',
			kind: 'meal.delete',
			at: '2026-10-29T12:00:00.000Z',
			payload: { id: 'm1' }
		};
		const result = overlayMeals(base, [queued(op)], '2026-10-29');
		expect(result.map((m) => m.id)).toEqual(['m2']);
	});

	it('applies sending ops but ignores stale, rejected and failed ones', () => {
		const ops = [
			queued(upsert('pending', [item('a', 100)]), 'pending'),
			queued(upsert('sending', [item('b', 200)]), 'sending'),
			queued(upsert('stale', [item('c', 300)]), 'stale'),
			queued(upsert('rejected', [item('d', 400)]), 'rejected'),
			queued(upsert('failed', [item('e', 500)]), 'failed')
		];
		const result = overlayMeals([], ops, '2026-10-29');
		expect(result.map((m) => m.id).sort()).toEqual(['pending', 'sending']);
	});

	it('ignores ops for another date', () => {
		const result = overlayMeals([], [queued(upsert('m1', [item('i1', 100)], '2026-10-28'))], '2026-10-29');
		expect(result).toEqual([]);
	});
});

describe('overlayDay', () => {
	it('merges present fields and leaves absent ones alone', () => {
		const op: Op = {
			op_id: 'p1',
			kind: 'day.patch',
			at: '2026-10-29T12:00:00.000Z',
			payload: { date: '2026-10-29', steps: 5000, weight_kg: 81.4 }
		};
		const result = overlayDay(day(), [queued(op)], '2026-10-29');
		expect(result.steps).toBe(5000);
		expect(result.steps_source).toBe('manual');
		expect(result.weight_kg).toBe(81.4);
		expect(result.gym_minutes).toBeNull();
		expect(result.trained).toEqual(['Legs']);
	});

	it('clears a field with null and marks it manual', () => {
		const op: Op = {
			op_id: 'p2',
			kind: 'day.patch',
			at: '2026-10-29T12:00:00.000Z',
			payload: { date: '2026-10-29', steps: null, trained: null }
		};
		const result = overlayDay(day(), [queued(op)], '2026-10-29');
		expect(result.steps).toBeNull();
		expect(result.steps_source).toBe('manual');
		expect(result.trained).toEqual([]);
	});

	it('does not relabel an unchanged automated value', () => {
		const op: Op = {
			op_id: 'p3',
			kind: 'day.patch',
			at: '2026-10-29T12:00:00.000Z',
			payload: { date: '2026-10-29', steps: 1000 }
		};
		const result = overlayDay(day(), [queued(op)], '2026-10-29');
		expect(result.steps).toBe(1000);
		expect(result.steps_source).toBe('google');
	});

	it('ignores patches for another date and non-live ops', () => {
		const otherDate: Op = {
			op_id: 'p4',
			kind: 'day.patch',
			at: '2026-10-29T12:00:00.000Z',
			payload: { date: '2026-10-28', steps: 1 }
		};
		const stale: Op = {
			op_id: 'p5',
			kind: 'day.patch',
			at: '2026-10-29T12:00:00.000Z',
			payload: { date: '2026-10-29', steps: 9999 }
		};
		const result = overlayDay(day(), [queued(otherDate), queued(stale, 'stale')], '2026-10-29');
		expect(result.steps).toBe(1000);
	});
});
