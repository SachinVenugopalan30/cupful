// SPDX-License-Identifier: AGPL-3.0-or-later
// Links an AI estimate item to a saved product with the same name, so Review can offer the swap.

import type { SavedProduct } from '#lib/api.ts';
import type { DraftItem } from './meal';

/** Trim, lowercase and collapse internal whitespace: the only name matching Review does. */
export const normalizeName = (name: string): string => name.trim().toLowerCase().replace(/\s+/g, ' ');

/**
 * The saved product whose name exactly matches this AI item, or null. Only AI estimate items that
 * are not swapped and not already this product are flagged; manual and database items never are.
 */
export function matchProduct(item: DraftItem, saved: SavedProduct[] | null): SavedProduct | null {
	if (!saved?.length || item.origin !== 'ai_estimate' || item.swapped) return null;
	const name = normalizeName(item.name);
	if (!name) return null;
	return saved.find((p) => p.food_id !== item.food_id && normalizeName(p.name) === name) ?? null;
}
