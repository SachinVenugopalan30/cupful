// SPDX-License-Identifier: AGPL-3.0-or-later
// Activity form helpers. Ranges mirror DayPatch::validate in server/src/sync.rs.

import type { ActivitySource } from '../api';
import { weightToKg, type WeightUnit } from '../format';

export const BODY_PARTS = ['Chest', 'Back', 'Legs', 'Shoulders', 'Arms', 'Core', 'Full body', 'Cardio'];

export const MAX_STEPS = 150_000;
export const MAX_GYM = 600;

/** `value: null` clears the field. */
export type Parsed = { value: number | null } | { error: string };

/** A whole number from 0 to `max`; empty clears. */
export function parseCount(text: string, max: number, name: string): Parsed {
	const t = text.trim();
	if (t === '') return { value: null };
	const n = Number(t);
	if (!Number.isInteger(n) || n < 0 || n > max) return { error: `${name} must be a whole number from 0 to ${max}.` };
	return { value: n };
}

/** A weight in the display unit, returned in kg (20 to 400 kg); empty clears. */
export function parseWeight(text: string, unit: WeightUnit): Parsed {
	const t = text.trim();
	if (t === '') return { value: null };
	const kg = weightToKg(Number(t), unit);
	if (!(kg >= 20 && kg <= 400)) return { error: 'Weight must be between 20 and 400 kg (44 and 880 lb).' };
	return { value: kg };
}

/** `list` with `part` added or removed, in the order of BODY_PARTS. */
export function toggle(list: string[], part: string): string[] {
	const next = list.includes(part) ? list.filter((p) => p !== part) : [...list, part];
	return BODY_PARTS.filter((p) => next.includes(p));
}

const SOURCES: Record<ActivitySource, string> = {
	manual: 'Manual',
	shortcut: 'Shortcut',
	import: 'Import',
	google: 'Google'
};
export const sourceLabel = (s: ActivitySource) => SOURCES[s];
