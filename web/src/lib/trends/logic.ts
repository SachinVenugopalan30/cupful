// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure helpers for the Trends screen.

import type { MealMix, TrendDay } from '../api';
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

/** Whole numbers with grouped thousands: 1,762. */
export const formatNumber = (n: number) => String(round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, ',');

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** An axis label in the render's style: "Oct 2". */
export function shortDate(iso: string): string {
	const [, month, day] = iso.split('-').map(Number);
	return `${MONTHS[month - 1]} ${day}`;
}

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

/** About `n` evenly spaced dates for an x axis, always including the first and last. */
export function tickDates(dates: string[], n: number): string[] {
	if (dates.length <= n) return dates;
	if (n <= 1) return [dates.at(-1)!];
	const out = [dates[0]];
	for (let i = 1; i < n - 1; i++) {
		out.push(dates[Math.round((i * (dates.length - 1)) / (n - 1))]);
	}
	out.push(dates.at(-1)!);
	return [...new Set(out)];
}

// ---- consistency ----

/**
 * `none` is a day with no meals. `unknown` has meals but no budget (before the first profile):
 * it is logged, but there is nothing to compare it with.
 */
export type ConsistencyKind = 'in' | 'over' | 'none' | 'unknown';

function consistencyKind(d: TrendDay): ConsistencyKind {
	if (d.eaten_kcal === null) return 'none';
	if (d.budget_kcal === null) return 'unknown';
	return d.eaten_kcal > d.budget_kcal ? 'over' : 'in';
}

export function consistency(days: TrendDay[]) {
	const cells = days.map((d) => ({ date: d.date, kind: consistencyKind(d) }));
	const count = (kind: ConsistencyKind) => cells.filter((c) => c.kind === kind).length;
	return {
		cells,
		in: count('in'),
		over: count('over'),
		none: count('none'),
		unknown: count('unknown'),
		logged: days.filter((d) => d.eaten_kcal !== null).length
	};
}

/** Columns for the consistency grid: one per visible day, capped at 30 so 365 days stay compact. */
export const consistencyColumns = (n: number) => Math.min(Math.max(n, 1), 30);

// ---- weight ----

/** Change between the first and latest weigh-in, plus the change over the trailing 30 days. */
export function weightStats(days: TrendDay[]) {
	const weighed = days.filter((d) => d.weight_kg !== null);
	const first = weighed[0]?.weight_kg ?? null;
	const latest = weighed.at(-1)?.weight_kg ?? null;
	const lastDate = days.at(-1)?.date;
	const cutoff = lastDate ? addDays(lastDate, -29) : '';
	const month = weighed.filter((d) => d.date >= cutoff);
	const monthFirst = month[0]?.weight_kg ?? null;
	const monthLatest = month.at(-1)?.weight_kg ?? null;
	return {
		count: weighed.length,
		first,
		latest,
		change: weighed.length > 1 ? latest! - first! : null,
		monthChange: month.length > 1 ? monthLatest! - monthFirst! : null
	};
}

/** A signed change in the display unit, with the render's minus sign: "−1.4 kg". */
export function signedWeightDelta(kg: number, unit: WeightUnit): string {
	const v = round(kg / (unit === 'kg' ? 1 : 0.45359237), 1);
	if (v === 0) return `0 ${unit}`;
	return `${v < 0 ? '−' : '+'}${formatWeight(Math.abs(kg), unit)}`;
}

/** What is left to the goal, in the display unit. Null without a goal. */
export function goalRemaining(
	latestKg: number | null,
	goalKg: number | null,
	unit: WeightUnit
): string | null {
	if (latestKg === null || goalKg === null) return null;
	const left = latestKg - goalKg;
	return left > 0 ? `${formatWeight(left, unit)} to goal` : 'Goal reached';
}

// ---- steps ----

/** Average, best day and the gap to the profile's typical day. */
export function stepsStats(days: TrendDay[], typical: number | null) {
	const steps = days.filter((d) => d.steps !== null).map((d) => d.steps!);
	const avg = mean(steps);
	return {
		count: steps.length,
		avg,
		best: steps.length ? Math.max(...steps) : null,
		diff: avg !== null && typical !== null ? avg - typical : null
	};
}

// ---- meal mix ----

export type MealShare = { key: keyof MealMix; label: string; kcal: number; pct: number };

const MEAL_ROWS: { key: keyof MealMix; label: string }[] = [
	{ key: 'breakfast', label: 'Breakfast' },
	{ key: 'lunch', label: 'Lunch' },
	{ key: 'dinner', label: 'Dinner' },
	{ key: 'snack', label: 'Snack' }
];

/** Whole percentages that still add up to 100, by largest remainder. */
function largestRemainder(values: number[], total: number): number[] {
	const floors = values.map((v) => Math.floor(v));
	let left = total - floors.reduce((a, b) => a + b, 0);
	const order = values
		.map((v, i) => ({ i, frac: v - Math.floor(v) }))
		.sort((a, b) => b.frac - a.frac);
	for (const { i } of order) {
		if (left <= 0) break;
		floors[i] += 1;
		left -= 1;
	}
	return floors;
}

/** Meal shares of the average eaten kcal. Null when there is no mix to show. */
export function mealShares(mix: MealMix | null): MealShare[] | null {
	if (!mix) return null;
	const kcal = MEAL_ROWS.map((r) => mix[r.key]);
	const total = kcal.reduce((a, b) => a + b, 0);
	if (total <= 0) return null;
	const pcts = largestRemainder(
		kcal.map((k) => (k / total) * 100),
		100
	);
	return MEAL_ROWS.map((row, i) => ({ ...row, kcal: kcal[i], pct: pcts[i] }));
}
