// SPDX-License-Identifier: AGPL-3.0-or-later
// Usuals: the one-tap repeat chip for the current meal slot. Pure selection and payload logic;
// dismissal is a local preference, not a server setting.

import type { HistoryMeal, MealType } from '#lib/api.ts';
import type { QueuedOp } from '#lib/offline/types.ts';
import { mealDelete, mealUpsert, type MealIn, type Op } from '#lib/sync/index.ts';
import { mealFromDraft, repeatDraft } from './meal.ts';

export const USUAL_MIN_TIMES = 3;
export const USUAL_DAYS = 60;
export const USUAL_LIMIT = 3;
export const USUAL_ALTERNATIVES = 2;

const DISMISSED_KEY = 'cupful.usuals.dismissed.v1';

type DismissStore = {
	getItem: (key: string) => string | null;
	setItem: (key: string, value: string) => void;
};

function localStore(): DismissStore | null {
	try {
		return typeof localStorage === 'undefined' ? null : localStorage;
	} catch {
		// A browser policy can block the getter itself, not just reads and writes.
		return null;
	}
}

/** Stable key for a slot and a normalized label. */
export function usualKey(mealType: MealType, label: string): string {
	return `${mealType}:${label.trim().replace(/\s+/g, ' ').toLowerCase()}`;
}

export function isDismissed(meal: HistoryMeal, dismissed: readonly string[]): boolean {
	return dismissed.includes(usualKey(meal.meal_type, meal.label));
}

const live = (entry: QueuedOp) => entry.status === 'pending' || entry.status === 'sending';

/** The server's label rule for a meal: its description, else its item names joined. */
function queuedLabel(meal: MealIn): string {
	const description = meal.description.trim();
	if (description) return description;
	return meal.items
		.map((i) => i.name.trim())
		.filter((n) => n !== '')
		.sort((a, b) => {
			const left = a.toLowerCase();
			const right = b.toLowerCase();
			return left < right ? -1 : left > right ? 1 : 0;
		})
		.join(', ');
}

/**
 * True when a still-queued log already records this usual for `date`. History reads come from the
 * cache, so without this the chip returns after an offline Log and a second tap duplicates the
 * meal. A queued delete of the same meal cancels it, so Undo brings the chip back.
 */
export function queuedLog(usual: HistoryMeal, date: string, ops: readonly QueuedOp[]): boolean {
	const deleted = new Set<string>();
	for (const entry of ops) {
		if (live(entry) && entry.op.kind === 'meal.delete') deleted.add(entry.op.payload.id);
	}
	const key = usualKey(usual.meal_type, usual.label);
	return ops.some((entry) => {
		if (!live(entry) || entry.op.kind !== 'meal.upsert') return false;
		const meal = entry.op.payload;
		if (deleted.has(meal.id) || meal.date !== date || meal.meal_type !== usual.meal_type) return false;
		return usualKey(meal.meal_type, queuedLabel(meal)) === key;
	});
}

/**
 * The top usual for the slot: frequent first (the server's order), skipping dismissed groups.
 * Nothing shows until it has been eaten three times, and it hides once it was logged today.
 */
export function pickUsual(
	meals: readonly HistoryMeal[],
	mealType: MealType,
	today: string,
	dismissed: readonly string[] = [],
	ops: readonly QueuedOp[] = []
): HistoryMeal | null {
	const top = meals.find((m) => m.meal_type === mealType && !isDismissed(m, dismissed));
	if (!top || top.times < USUAL_MIN_TIMES) return null;
	if (top.last_date === today || queuedLog(top, today, ops)) return null;
	return top;
}

/** The other usuals for the slot, for the menu. */
export function usualAlternatives(
	meals: readonly HistoryMeal[],
	mealType: MealType,
	top: HistoryMeal,
	dismissed: readonly string[] = []
): HistoryMeal[] {
	if (!top) return [];
	const key = usualKey(top.meal_type, top.label);
	return meals
		.filter(
			(m) =>
				m.meal_type === mealType &&
				m.times >= USUAL_MIN_TIMES &&
				!isDismissed(m, dismissed) &&
				usualKey(m.meal_type, m.label) !== key
		)
		.slice(0, USUAL_ALTERNATIVES);
}

export function readDismissed(store: DismissStore | null = localStore()): string[] {
	try {
		const value = JSON.parse(store?.getItem(DISMISSED_KEY) ?? '[]');
		return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
	} catch {
		return [];
	}
}

export function dismissUsual(
	mealType: MealType,
	label: string,
	current: readonly string[] = [],
	store: DismissStore | null = localStore()
): string[] {
	// Merge the caller's list too: a failed write must not resurrect an earlier dismissal.
	const next = [...new Set([...current, ...readDismissed(store), usualKey(mealType, label)])];
	try {
		store?.setItem(DISMISSED_KEY, JSON.stringify(next));
	} catch {
		// Storage can be full or blocked; the chip still hides for this page.
	}
	return next;
}

/** A fresh repeat on `date`, ready for `meal.upsert`. */
export function usualMeal(usual: HistoryMeal, date: string): MealIn {
	return mealFromDraft(repeatDraft(usual, date));
}

export function usualLogOp(usual: HistoryMeal, date: string): Op {
	return mealUpsert(usualMeal(usual, date));
}

export function usualUndoOp(id: string): Op {
	return mealDelete(id);
}
