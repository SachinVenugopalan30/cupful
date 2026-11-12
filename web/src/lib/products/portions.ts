// SPDX-License-Identifier: AGPL-3.0-or-later
// Household portions shipped with a product: pick one, show fractions, and turn a count into grams.

import { formatGrams, round } from '../format.ts';

import type { ProductPortion } from '../api.ts';

export type { ProductPortion };

/** Shown first in the picker, then other named units, then "serving". */
const UNIT_ORDER = ['cup', 'tbsp', 'tsp', 'scoop', 'serving'];

const rank = (unit: string) => {
	const i = UNIT_ORDER.indexOf(unit.trim().toLowerCase());
	return i === -1 ? UNIT_ORDER.length : i;
};

const isServing = (unit: string) => unit.trim().toLowerCase() === 'serving';
const INVARIANT = new Set(['tsp', 'tbsp', 'g', 'ml', 'oz', 'fl oz', 'kg', 'lb']);

/** "2 scoops", but "½ cup" and "2 tsp". */
function unitLabel(quantity: number, unit: string): string {
	return quantity > 1 && !INVARIANT.has(unit.toLowerCase()) ? `${unit}s` : unit;
}

/** Rows with a unit and positive grams, sorted for the picker. */
function valid(portions: readonly ProductPortion[]): ProductPortion[] {
	return portions.filter((p) => p.unit.trim() !== '' && Number.isFinite(p.grams) && p.grams > 0);
}

/** Picker options: de-duplicated by grams (a named unit beats "serving"), in display order. */
export function portionOptions(portions: readonly ProductPortion[]): ProductPortion[] {
	const byGrams = new Map<number, ProductPortion>();
	for (const p of valid(portions)) {
		const key = round(p.grams, 1);
		const kept = byGrams.get(key);
		if (!kept || (isServing(kept.unit) && !isServing(p.unit))) byGrams.set(key, p);
	}
	return [...byGrams.values()].sort((a, b) => rank(a.unit) - rank(b.unit) || a.unit.localeCompare(b.unit));
}

/** The one-unit portion a new product row starts on. */
export function bestPortion(portions: readonly ProductPortion[]): ProductPortion | null {
	return portionOptions(portions)[0] ?? null;
}

/** "1.5" -> "1½"; simple fractions as glyphs, anything else at up to two decimals. */
export function formatQuantity(q: number): string {
	if (!Number.isFinite(q)) return String(q);
	const whole = Math.floor(q + 1e-9);
	const frac = round(q - whole, 2);
	const glyph = { 0.25: '¼', 0.33: '⅓', 0.5: '½', 0.67: '⅔', 0.75: '¾' }[frac];
	if (glyph) return `${whole > 0 ? whole : ''}${glyph}`;
	return String(round(q, 2));
}

/** "½ cup (40 g)", "2 tsp (10 g)", "1 scoop (31 g)"; liquids use ml. */
export function portionLabel(quantity: number, portion: ProductPortion, unit: 'g' | 'ml' = 'g'): string {
	return `${formatQuantity(quantity)} ${unitLabel(quantity, portion.unit)} (${formatGrams(portionGrams(quantity, portion))} ${unit})`;
}

/**
 * The label for a stored amount: the household label, or the grams alone when the count rounds
 * away. "1 g" of a 240 g cup reads better than "0 cups (1 g)" and stays true to the grams.
 */
export function amountLabel(
	quantity: number,
	grams: number,
	portion: ProductPortion,
	unit: 'g' | 'ml' = 'g'
): string {
	return round(quantity, 2) > 0 ? portionLabel(quantity, portion, unit) : `${formatGrams(grams)} ${unit}`;
}

/** Quantity times grams per unit, rounded like the rest of the UI. */
export function portionGrams(quantity: number, portion: ProductPortion): number {
	return round(quantity * portion.grams, 1);
}
