// SPDX-License-Identifier: AGPL-3.0-or-later
// The jug is a picture of eaten / budget. The server owns the budget; this only scales it.

import { formatKcal } from '#lib/format.ts';

/** Liquid height, 0 to 1. Over budget the jug is full; the figures say how far over. */
export function jugFraction(eaten: number, budget: number): number {
	if (!Number.isFinite(eaten) || !Number.isFinite(budget) || budget <= 0) return 0;
	return Math.min(Math.max(eaten / budget, 0), 1);
}

export type JugMark = { at: number; label: string };

/** Graduations at quarters of the budget, top first, labelled to the nearest 50 kcal. */
export function jugMarks(budget: number): JugMark[] {
	if (!Number.isFinite(budget) || budget <= 0) return [];
	return [1, 0.75, 0.5, 0.25].map((at) => ({
		at,
		// Printed like a real measuring jug's marks: no thousands separator.
		label: String(Math.round((budget * at) / 50) * 50)
	}));
}

/** The jug's accessible name; the visible figures stay real text. */
export function jugLabel(eaten: number, budget: number, manual: boolean): string {
	const over = budget > 0 && Math.round(eaten) > Math.round(budget);
	return `${formatKcal(eaten)} of ${formatKcal(budget)} kcal ${manual ? 'goal' : 'plan'} eaten${over ? ', over budget' : ''}`;
}
