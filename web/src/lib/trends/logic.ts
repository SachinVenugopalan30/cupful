// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure helpers for the Trends screen.

import type { TrendDay } from '../api';
import { addDays, dayLabel } from '../date';
import { formatWeight, round, type WeightUnit } from '../format';

export const RANGES = [7, 30, 90, 365] as const;
export type Range = (typeof RANGES)[number];

const KEY = 'cupful-trends-range';

export function loadRange(storage: Storage = localStorage): Range {
	try {
		const v = Number(storage.getItem(KEY));
		return RANGES.find((r) => r === v) ?? 30;
	} catch {
		return 30;
	}
}

export function saveRange(range: Range, storage: Storage = localStorage): void {
	try {
		storage.setItem(KEY, String(range));
	} catch {
		// Private mode: the choice lasts for this page only.
	}
}

/** The last `n` days ending at `today`, inclusive. */
export function rangeDates(n: number, today: string): { from: string; to: string } {
	return { from: addDays(today, -(n - 1)), to: today };
}

/** Mean of the values present in the trailing `window` entries; null where the window has none. */
export function rollingAverage(values: (number | null)[], window = 7): (number | null)[] {
	return values.map((_, i) => {
		const seen = values.slice(Math.max(0, i - window + 1), i + 1).filter((v) => v !== null);
		return seen.length ? seen.reduce((a, b) => a + b, 0) / seen.length : null;
	});
}

const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null);

/**
 * Averages for the summary. Intake and budget are both taken over the days that have meals, so
 * they compare like with like; a day with nothing logged is unknown, not zero.
 */
export function averages(days: TrendDay[]) {
	const logged = days.filter((d) => d.eaten_kcal !== null);
	return {
		days: logged.length,
		intake: mean(logged.map((d) => d.eaten_kcal!)),
		budget: mean(logged.filter((d) => d.budget_kcal !== null).map((d) => d.budget_kcal!)),
		steps: mean(days.filter((d) => d.steps !== null).map((d) => d.steps!))
	};
}

// ---- words for the line under a chart title (tap) ----

const kcal = (n: number) => `${round(n)} kcal`;

/** What a tapped day says about intake: eaten, budget and the difference, in words. */
export function intakeReadout(d: TrendDay, today: string): string {
	const day = dayLabel(d.date, today);
	if (d.eaten_kcal === null) return `${day}: no meals logged.`;
	if (d.budget_kcal === null) return `${day}: ${kcal(d.eaten_kcal)} eaten.`;
	const diff = round(d.budget_kcal - d.eaten_kcal);
	const rest = diff >= 0 ? `${diff} left` : `${-diff} over`;
	return `${day}: ${kcal(d.eaten_kcal)} eaten of ${kcal(d.budget_kcal)}, ${rest}.`;
}

export function weightReadout(d: TrendDay, avg: number | null, today: string, unit: WeightUnit): string {
	const day = dayLabel(d.date, today);
	const parts = [];
	if (d.weight_kg !== null) parts.push(`${formatWeight(d.weight_kg, unit)} weighed in`);
	if (avg !== null) parts.push(`${formatWeight(avg, unit)} 7-day average`);
	return parts.length ? `${day}: ${parts.join(', ')}.` : `${day}: no weigh-in.`;
}

export function stepsReadout(d: TrendDay, today: string): string {
	const day = dayLabel(d.date, today);
	const gym = d.gym_minutes ? `, gym ${d.gym_minutes} min` : '';
	return d.steps === null ? `${day}: no steps recorded${gym}.` : `${day}: ${d.steps} steps${gym}.`;
}

/** About `n` evenly spaced dates for an x axis, always including the last. */
export function tickDates(dates: string[], n: number): string[] {
	if (dates.length <= n) return dates;
	const step = Math.ceil((dates.length - 1) / (n - 1));
	const out: string[] = [];
	for (let i = dates.length - 1; i >= 0; i -= step) out.unshift(dates[i]);
	return out;
}
