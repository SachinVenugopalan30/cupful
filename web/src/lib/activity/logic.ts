// SPDX-License-Identifier: AGPL-3.0-or-later
// Activity form helpers. Ranges mirror DayPatch::validate in server/src/sync.rs.

import type { ActivitySource, Day, Workout } from '../api';
import { addDays } from '../date';
import { round, weightToKg, type WeightUnit } from '../format';

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

export const sourceLabel = (s: ActivitySource) => s[0].toUpperCase() + s.slice(1);

/** What the cards and the week strip share: day rows (30 days) and workouts (7) ending on `date`. */
export type ActivityWindow = { date: string; days: Day[]; workouts: Workout[] };

/** Whether `date` falls inside the window fetched for `win.date`, even if that day has no row. */
export function windowCovers(win: ActivityWindow, date: string, span: number): boolean {
	return date <= win.date && date >= addDays(win.date, -(span - 1));
}

// ---- the week around the selected day ----

/** The 7 dates ending on `date`, oldest first. */
export const weekWindow = (date: string): string[] => Array.from({ length: 7 }, (_, i) => addDays(date, i - 6));

/** Whole percent of the typical day; null without steps or a usable typical figure. */
export function stepsVsTypical(steps: number | null, typical: number | null): number | null {
	if (steps === null || !typical || typical <= 0) return null;
	return Math.round((steps / typical) * 100);
}

/** Mean of the logged steps on `dates` (a day with none is unknown, not zero). */
export function stepsAverage(days: Day[], dates: string[]): number | null {
	const seen = days.filter((d) => dates.includes(d.date) && d.steps !== null).map((d) => d.steps!);
	return seen.length ? seen.reduce((a, b) => a + b, 0) / seen.length : null;
}

/** Workouts keyed by date. */
export function groupByDate(workouts: Workout[]): Map<string, Workout[]> {
	const out = new Map<string, Workout[]>();
	for (const w of workouts) out.set(w.date, [...(out.get(w.date) ?? []), w]);
	return out;
}

/** Dates that count as a gym session: gym minutes above 0, or a workout counted as gym. */
export function gymDates(days: Day[], workouts: Workout[]): Set<string> {
	const out = new Set<string>();
	for (const d of days) if ((d.gym_minutes ?? 0) > 0) out.add(d.date);
	for (const w of workouts) if (w.counts_as_gym) out.add(w.date);
	return out;
}

/** Gym days among `dates`. */
export const sessionsIn = (gym: Set<string>, dates: string[]) => dates.filter((d) => gym.has(d)).length;

/** The latest gym day on or before `date`, or null. */
export function lastTrained(gym: Set<string>, date: string): string | null {
	return [...gym].filter((d) => d <= date).sort().at(-1) ?? null;
}

/** The latest weigh-in on or before `date` and the one before it; delta is in kg. */
export function weightDelta(days: Day[], date: string) {
	const weighed = days.filter((d) => d.weight_kg !== null && d.date <= date).sort((a, b) => a.date.localeCompare(b.date));
	const last = weighed.at(-1);
	const prev = weighed.at(-2);
	if (!last || !prev) return null;
	return { date: last.date, prevDate: prev.date, deltaKg: last.weight_kg! - prev.weight_kg! };
}

/** The latest weigh-in on or before `date`, or null. */
export function lastWeight(days: Day[], date: string): { kg: number; date: string } | null {
	const last = days
		.filter((d) => d.weight_kg !== null && d.date <= date)
		.sort((a, b) => a.date.localeCompare(b.date))
		.at(-1);
	return last ? { kg: last.weight_kg!, date: last.date } : null;
}

/**
 * SVG polyline points for the non-null values, spread over `w` x `h` with `pad` inside. The line
 * joins across gaps. Fewer than two points, or a flat series, still draw (flat sits mid-height).
 */
export function sparklinePoints(values: (number | null)[], w: number, h: number, pad = 3): string {
	const pts = values.map((v, i) => [i, v] as const).filter((p): p is readonly [number, number] => p[1] !== null);
	if (pts.length < 2) return '';
	const lo = Math.min(...pts.map((p) => p[1]));
	const hi = Math.max(...pts.map((p) => p[1]));
	const x = (i: number) => pad + (i / Math.max(values.length - 1, 1)) * (w - 2 * pad);
	const y = (v: number) => (hi === lo ? h / 2 : h - pad - ((v - lo) / (hi - lo)) * (h - 2 * pad));
	return pts.map(([i, v]) => `${round(x(i), 1)},${round(y(v), 1)}`).join(' ');
}

// ---- workouts ----

/** Metres as km, one decimal: "5.2 km". The app has no distance unit setting. */
export const formatKm = (m: number) => `${round(m / 1000, 1)} km`;

/** "31 min", "1 h 05 min". */
export function formatDuration(minutes: number): string {
	const m = Math.round(minutes);
	return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} min`;
}

/** "7:42 am" in `tz`. */
export function formatClock(iso: string, tz: string): string {
	return new Intl.DateTimeFormat('en-GB', { timeZone: tz, hour: 'numeric', minute: '2-digit', hour12: true })
		.format(new Date(iso))
		.replace(/\s/g, ' ');
}

export type WorkoutKind = 'run' | 'walk' | 'bike' | 'swim' | 'strength' | 'other';
/** Which icon a workout type gets. */
export function workoutKind(type: string): WorkoutKind {
	const t = type.toUpperCase();
	if (/RUN|JOG/.test(t)) return 'run';
	if (/WALK|HIK/.test(t)) return 'walk';
	if (/BIK|CYCL/.test(t)) return 'bike';
	if (/SWIM/.test(t)) return 'swim';
	if (/STRENGTH|WEIGHT|GYM|CROSSFIT/.test(t)) return 'strength';
	return 'other';
}
