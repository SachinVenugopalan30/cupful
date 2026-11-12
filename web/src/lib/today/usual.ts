// SPDX-License-Identifier: AGPL-3.0-or-later
// Usuals: the one-tap repeat chip for the current meal slot. Pure selection and payload logic;
// dismissal is a local preference, not a server setting.

import type { HistoryMeal, MealType } from '#lib/api.ts';
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
	return typeof localStorage === 'undefined' ? null : localStorage;
}

/** Stable key for a slot and a normalized label. */
export function usualKey(mealType: MealType, label: string): string {
	return `${mealType}:${label.trim().replace(/\s+/g, ' ').toLowerCase()}`;
}

export function isDismissed(meal: HistoryMeal, dismissed: readonly string[]): boolean {
	return dismissed.includes(usualKey(meal.meal_type, meal.label));
}

/**
 * The top usual for the slot: frequent first (the server's order), skipping dismissed groups.
 * Nothing shows until it has been eaten three times, and it hides once it was logged today.
 */
export function pickUsual(
	meals: readonly HistoryMeal[],
	mealType: MealType,
	today: string,
	dismissed: readonly string[] = []
): HistoryMeal | null {
	const top = meals.find((m) => m.meal_type === mealType && !isDismissed(m, dismissed));
	if (!top || top.times < USUAL_MIN_TIMES || top.last_date === today) return null;
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
	store: DismissStore | null = localStore()
): string[] {
	const next = [...new Set([...readDismissed(store), usualKey(mealType, label)])];
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
