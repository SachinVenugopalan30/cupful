// SPDX-License-Identifier: AGPL-3.0-or-later
// Display formatting. The server already rounds what it computes; these format raw client sums
// and user units. Rounding matches the server: half away from zero.

export type WeightUnit = 'kg' | 'lb';
export type HeightUnit = 'cm' | 'ftin';

const KG_PER_LB = 0.45359237;
const CM_PER_IN = 2.54;

/** Half away from zero, at `digits` decimals; never returns -0. */
function round(x: number, digits = 0): number {
	const f = 10 ** digits;
	return Math.sign(x) * Math.round(Math.abs(x) * f) / f || 0;
}

/** Whole kcal: "1235". */
export function formatKcal(kcal: number): string {
	return String(round(kcal));
}

/** Grams to one decimal, without a trailing ".0": "12.3", "150". */
export function formatGrams(g: number): string {
	return String(round(g, 1));
}

export function formatWeight(kg: number, unit: WeightUnit): string {
	return unit === 'kg' ? `${round(kg, 1)} kg` : `${round(kg / KG_PER_LB, 1)} lb`;
}

export function weightToKg(value: number, unit: WeightUnit): number {
	return unit === 'kg' ? value : value * KG_PER_LB;
}

export function formatHeight(cm: number, unit: HeightUnit): string {
	if (unit === 'cm') return `${round(cm)} cm`;
	const inches = round(cm / CM_PER_IN);
	return `${Math.floor(inches / 12)}′ ${inches % 12}″`;
}

export function heightToCm(feet: number, inches: number): number {
	return (feet * 12 + inches) * CM_PER_IN;
}
