// SPDX-License-Identifier: AGPL-3.0-or-later
// What the budget card shows. The server computes the budget (PLAN.md §7); this only sums what
// was eaten and puts the server's numbers into words.

import type { Budget, BudgetFlag, Macros, Meal } from '../api';
import { formatWeight, type WeightUnit } from '../format';

const FLAGS: Record<BudgetFlag, string> = {
	provisional: 'No steps logged yet — using your typical steps. The budget updates when you add them.',
	deficit_capped: "Your weekly pace would cut more than 25 % of maintenance, so today's deficit is capped at 25 %.",
	floor_applied: 'The budget is raised to the daily minimum (the floor), so the deficit is smaller today.',
	floor_above_maintenance:
		"The minimum floor is above what you burn today, so no deficit is planned today.",
	macros_overcommitted:
		"Protein takes up most of today's budget, so there's no room set aside for carbs. Fat gets what's left."
};

export const flagMessages = (flags: BudgetFlag[]) => flags.map((f) => FLAGS[f]);

export function eaten(meals: Meal[]): Macros {
	const sum: Macros = { kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 };
	for (const { totals } of meals) for (const k of Object.keys(sum) as (keyof Macros)[]) sum[k] += totals[k];
	return sum;
}

/** Whole kcal left (or over) of `budget`. Rounded first, so 0.2 over reads "0 left". */
export function leftOver(eatenKcal: number, budget: number) {
	const diff = Math.round(budget - eatenKcal);
	return { over: diff < 0, kcal: Math.abs(diff) };
}

export type MacroRow = {
	key: 'protein_g' | 'carbs_g' | 'fat_g';
	label: string;
	eaten: number;
	/** null: no target today (carbs when protein over-commits the budget). */
	target: number | null;
	value: number;
	over: boolean;
};

export function macroRows(b: Budget, e: Macros): MacroRow[] {
	const noCarbs = b.flags.includes('macros_overcommitted');
	return (
		[
			['protein_g', 'Protein'],
			['carbs_g', 'Carbs'],
			['fat_g', 'Fat']
		] as const
	).map(([key, label]) => {
		const target = key === 'carbs_g' && noCarbs ? null : b[key];
		const value = target ? e[key] / target : 0;
		return { key, label, eaten: e[key], target, value, over: value > 1 };
	});
}

const n = (x: number) => x.toLocaleString('en-US');

/** Every term of the server's calculation, in order, with a one-line explanation. */
export function breakdown(b: Budget, unit: WeightUnit = 'kg') {
	return [
		{ label: 'Weight used', value: formatWeight(b.weight_kg, unit), note: 'Your latest weigh-in on or before this day, else your start weight.' },
		{ label: 'Age', value: String(b.age), note: 'This year minus your birth year (can be a year off).' },
		{ label: 'BMR', value: `${n(b.bmr)} kcal`, note: 'Energy at rest, from the Mifflin–St Jeor formula.' },
		{ label: 'Base', value: `${n(b.base)} kcal`, note: 'BMR × 1.2, for everyday light movement.' },
		{ label: 'Steps used', value: n(b.steps), note: "This day's steps, or your typical steps if none are logged." },
		{ label: 'Steps kcal', value: `${n(b.steps_kcal)} kcal`, note: 'Extra burn from steps beyond the first 2,500.' },
		{ label: 'MET', value: String(b.met), note: 'Workout intensity: 5 for weight training, 7 for cardio only.' },
		{ label: 'Gym kcal', value: `${n(b.gym_kcal)} kcal`, note: 'Extra burn from gym minutes at that intensity.' },
		{ label: 'Maintenance', value: `${n(b.maintenance)} kcal`, note: 'Base + steps + gym: what keeps your weight steady.' },
		{ label: 'Deficit', value: `${n(b.deficit)} kcal`, note: 'Your weekly pace as a daily cut, at most 25 % of maintenance.' },
		{ label: 'Floor', value: `${n(b.floor)} kcal`, note: 'A conventional minimum the budget never goes below.' },
		{ label: 'Budget', value: `${n(b.budget)} kcal`, note: 'Maintenance minus the deficit, kept at or above the floor.' }
	];
}
