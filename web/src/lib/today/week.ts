// SPDX-License-Identifier: AGPL-3.0-or-later
// Today's side cards: the "This week" bars and the steps tile. Display shaping only; the numbers
// come from the server (GET /api/trends and the day read).

import type { TrendDay } from '../api';
import { addDays } from '../date.ts';
import { formatKcal } from '../format.ts';

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** The seven days ending `today`, for GET /api/trends. */
export function weekRange(today: string): { from: string; to: string } {
	return { from: addDays(today, -6), to: today };
}

export type WeekBar = {
	date: string;
	label: string;
	/** kcal eaten; null when nothing was logged. */
	kcal: number | null;
	/** Bar height, 0 to 1 of the chart. */
	height: number;
	/** Where the plan line sits, 0 to 1. The same for every bar. */
	planAt: number;
	over: boolean;
	today: boolean;
};

/**
 * Seven bars ending today. Today follows `todayEaten` (the live figure the jug shows), older days
 * come from the trends rows. A day is over when it ate more than its own budget, else the plan.
 */
export function weekBars(
	days: readonly TrendDay[],
	today: string,
	todayEaten: number,
	plan: number
): WeekBar[] {
	const byDate = new Map(days.map((d) => [d.date, d]));
	const rows = Array.from({ length: 7 }, (_, i) => {
		const date = addDays(today, i - 6);
		const row = byDate.get(date);
		const isToday = date === today;
		const kcal = isToday ? (todayEaten > 0 ? todayEaten : null) : (row?.eaten_kcal ?? null);
		const budget = row?.budget_kcal ?? plan;
		const [y, m, d] = date.split('-').map(Number);
		return {
			date,
			kcal,
			isToday,
			over: kcal !== null && budget > 0 && kcal > (isToday ? plan : budget),
			label: WEEKDAYS[new Date(Date.UTC(y, m - 1, d)).getUTCDay()]
		};
	});
	const scale = Math.max(plan * 1.25, ...rows.map((r) => r.kcal ?? 0), 1);
	return rows.map((r) => ({
		date: r.date,
		label: r.label,
		kcal: r.kcal,
		height: r.kcal === null ? 0 : r.kcal / scale,
		planAt: plan / scale,
		over: r.over,
		today: r.isToday
	}));
}

/** "Fri 1,830 kcal" for the bar's accessible name and tooltip. */
export const barText = (b: WeekBar) =>
	`${b.today ? 'Today' : b.label} ${b.kcal === null ? 'nothing logged' : `${formatKcal(b.kcal)} kcal`}`;

/** The steps tile: a meter against typical steps (there is no step goal) and its caption. */
export function stepsTile(steps: number, typical: number): { value: number; label: string } {
	if (!(typical > 0)) return { value: 0, label: 'steps' };
	return {
		value: Math.min(steps / typical, 1),
		label: `of ${typical.toLocaleString('en-US')} typical`
	};
}
