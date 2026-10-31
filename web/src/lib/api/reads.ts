// SPDX-License-Identifier: AGPL-3.0-or-later
// Reads: health, days, budget, trends, meals. User data goes through the offline cache and gets
// queued writes applied on top.

import { api, cachedGet, enc } from './http.ts';
import type { Budget, Day, Health, Meal, Trends } from './types.ts';
import { overlayDay, overlayMeals } from '../offline/overlay.ts';
import { queueState } from '../offline/state.svelte.ts';

export const health = () => api.get<Health>('/api/health');

export const getBudget = (date: string) => cachedGet<Budget>(`/api/budget/${enc(date)}`);

export const getTrends = (from: string, to: string) =>
	cachedGet<Trends>(`/api/trends?from=${enc(from)}&to=${enc(to)}`);

export async function getMeals(date: string): Promise<Meal[]> {
	const body = await cachedGet<{ meals: Meal[] }>(`/api/meals?date=${enc(date)}`);
	return overlayMeals(body.meals, queueState.ops, date);
}

/** Shows the day with any queued step/gym/weight patches already applied. */
export async function getDay(date: string): Promise<Day> {
	const body = await cachedGet<Day>(`/api/days/${enc(date)}`);
	return overlayDay(body, queueState.ops, date);
}
