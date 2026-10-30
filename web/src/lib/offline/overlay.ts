// SPDX-License-Identifier: AGPL-3.0-or-later
// Optimistic overlays: apply still-queued ops on top of server reads so the UI shows what the
// user just did before it syncs. Only `pending` and `sending` ops count; stale/rejected/failed
// ops are not shown as fact.

import type { Day, Macros, Meal } from '../api.ts';
import type { Op } from '../sync/index.ts';
import type { QueuedOp } from './types.ts';

function live(entry: QueuedOp): boolean {
	return entry.status === 'pending' || entry.status === 'sending';
}

function totals(items: Meal['items']): Macros {
	const sum: Macros = { kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 };
	for (const item of items) {
		sum.kcal += item.kcal;
		sum.protein_g += item.protein_g;
		sum.carbs_g += item.carbs_g;
		sum.fat_g += item.fat_g;
	}
	return sum;
}

/** Applies queued meal upserts and deletes for `date`; other dates are left alone. */
export function overlayMeals(meals: Meal[], ops: QueuedOp[], date: string): Meal[] {
	let result = meals.map((meal) => ({ ...meal, items: [...meal.items], totals: { ...meal.totals } }));

	for (const entry of ops) {
		if (!live(entry)) continue;
		const op = entry.op;
		if (op.kind === 'meal.delete') {
			result = result.filter((meal) => meal.id !== op.payload.id);
			continue;
		}
		if (op.kind !== 'meal.upsert') continue;

		const payload = op.payload;
		if (payload.date !== date) continue;

		const at = Date.parse(op.at);
		const updated_at = Number.isNaN(at) ? Date.now() : at;
		const items = payload.items.map((item) => ({ ...item }));
		const index = result.findIndex((meal) => meal.id === payload.id);
		const next: Meal = {
			...payload,
			items,
			totals: totals(items),
			updated_at
		};
		if (index === -1) result.push(next);
		else result[index] = next;
	}

	return result;
}

/** Applies queued day patches for `date`. Field changes made by hand become source `manual`. */
export function overlayDay(day: Day, ops: QueuedOp[], date: string): Day {
	const next: Day = { ...day, trained: [...day.trained] };

	for (const entry of ops) {
		if (!live(entry)) continue;
		const op = entry.op;
		if (op.kind !== 'day.patch' || op.payload.date !== date) continue;
		const patch = op.payload;

		if ('steps' in patch) {
			const value = patch.steps ?? null;
			if (value !== next.steps) next.steps_source = 'manual';
			next.steps = value;
		}
		if ('gym_minutes' in patch) {
			const value = patch.gym_minutes ?? null;
			if (value !== next.gym_minutes) next.gym_source = 'manual';
			next.gym_minutes = value;
		}
		if (patch.trained !== undefined) {
			next.trained = patch.trained === null ? [] : [...patch.trained];
		}
		if ('weight_kg' in patch) {
			next.weight_kg = patch.weight_kg ?? null;
		}
	}

	return next;
}
