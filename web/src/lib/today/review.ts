// SPDX-License-Identifier: AGPL-3.0-or-later
// Review-sheet logic: the per-item number inputs and the "always use this" aliases saved after a meal.

import type { DraftItem, NumKey } from './meal';

/** The number inputs behind "Edit numbers", in display order. */
export const MACROS: [NumKey, string][] = [
	['kcal', 'kcal'],
	['protein_g', 'Protein g'],
	['carbs_g', 'Carbs g'],
	['fat_g', 'Fat g']
];

/** Items the user asked to remember, with the phrase and food needed for an alias. */
function rememberTargets(items: DraftItem[]) {
	return items.filter((i) => i.remember && i.food_id !== null && i.phrase.trim());
}

/**
 * Saves an alias for each item flagged "remember", clearing the flag where it worked. Returns the
 * phrases that failed, so the sheet can name them; a failure leaves the meal saved either way.
 */
export async function rememberAliases(
	items: DraftItem[],
	alias: (phrase: string, foodId: number) => Promise<unknown>
): Promise<string[]> {
	const targets = rememberTargets(items);
	const results = await Promise.allSettled(targets.map((i) => alias(i.phrase.trim(), i.food_id!)));
	const failed = targets.filter((_, k) => results[k].status === 'rejected');
	for (const i of targets) if (!failed.includes(i)) i.remember = false;
	return failed.map((i) => i.phrase);
}

/** What the sheet says when some phrases couldn't be remembered. */
export function aliasFailure(phrases: string[]): string {
	const named = phrases.map((p) => `"${p}"`).join(', ');
	return `The meal is saved, but I couldn't remember ${named}. Save again to retry, or close.`;
}

/**
 * The quantity after a stepper press. 2 and up moves by 1, a quarter to 2 by a quarter; an off-grid
 * typed value first lands on the next step in the direction pressed. Never below a quarter.
 */
export function stepQuantity(quantity: number, direction: 1 | -1): number {
	// Up from 2 moves by 1; down to 2 or below moves by a quarter, so 1.75 ↔ 2 retrace each other.
	const step = (direction > 0 ? quantity >= 2 : quantity > 2) ? 1 : 0.25;
	const at = quantity / step;
	const next =
		direction > 0 ? (Math.floor(at + 1e-9) + 1) * step : (Math.ceil(at - 1e-9) - 1) * step;
	return Math.max(0.25, Math.round(next * 100) / 100);
}
