// SPDX-License-Identifier: AGPL-3.0-or-later
// The profile form: shows metric values in the user's units and turns them back into ProfileIn.
// Ranges mirror server/src/profile.rs.

import type { Profile, ProfileIn, Sex } from '../api';
import { heightToCm, round, weightToKg, type HeightUnit, type WeightUnit } from '../format';

export type ProfileForm = {
	timezone: string;
	weight_unit: WeightUnit;
	height_unit: HeightUnit;
	sex: Sex | '';
	birth_year: number | null;
	height_cm: number | null;
	height_ft: number | null;
	height_in: number | null;
	/** Weights and pace are in `weight_unit` (pace per week). */
	start_weight: number | null;
	goal_weight: number | null;
	pace: number | null;
	typical_steps: number | null;
};
export type ProfileErrors = Partial<Record<keyof ProfileForm | 'height', string>>;

const fromKg = (kg: number | null | undefined, u: WeightUnit, d = 1) =>
	kg == null ? null : round(kg / weightToKg(1, u), d);
const toKg = (v: number | null, u: WeightUnit) => (v == null ? null : weightToKg(v, u));
const paceDigits = (u: WeightUnit) => (u === 'kg' ? 2 : 1);

function setHeight(f: ProfileForm, cm: number | null): ProfileForm {
	if (cm == null) return { ...f, height_cm: null, height_ft: null, height_in: null };
	const inches = round(cm / heightToCm(0, 1), 1);
	return { ...f, height_cm: Math.round(cm), height_ft: Math.floor(inches / 12), height_in: round(inches % 12, 1) };
}

function heightCm(f: ProfileForm): number | null {
	if (f.height_unit === 'cm') return f.height_cm;
	return f.height_ft == null && f.height_in == null ? null : heightToCm(f.height_ft ?? 0, f.height_in ?? 0);
}

export function toForm(p: Profile, browserTimezone: string): ProfileForm {
	const v = p.version;
	const u = p.weight_unit;
	return setHeight(
		{
			timezone: p.timezone || browserTimezone,
			weight_unit: u,
			height_unit: p.height_unit,
			sex: v?.sex ?? '',
			birth_year: v?.birth_year ?? null,
			height_cm: null,
			height_ft: null,
			height_in: null,
			start_weight: fromKg(v?.start_weight_kg, u),
			goal_weight: fromKg(v?.goal_kg, u),
			pace: fromKg(v?.pace_kg_per_week ?? 0.5, u, paceDigits(u)),
			typical_steps: v?.typical_steps ?? 7000
		},
		v?.height_cm ?? null
	);
}

export function setWeightUnit(f: ProfileForm, u: WeightUnit): ProfileForm {
	const from = f.weight_unit;
	return {
		...f,
		weight_unit: u,
		start_weight: fromKg(toKg(f.start_weight, from), u),
		goal_weight: fromKg(toKg(f.goal_weight, from), u),
		pace: fromKg(toKg(f.pace, from), u, paceDigits(u))
	};
}

export function setHeightUnit(f: ProfileForm, u: HeightUnit): ProfileForm {
	return setHeight({ ...f, height_unit: u }, heightCm(f));
}

/** ProfileIn when every field is valid, else messages per field in the user's units. */
export function fromForm(f: ProfileForm, thisYear: number): { value?: ProfileIn; errors: ProfileErrors } {
	const errors: ProfileErrors = {};
	const u = f.weight_unit;
	const w = (kg: number) => `${fromKg(kg, u)} ${u}`;
	const inKg = (v: number | null, lo: number, hi: number) => v != null && weightToKg(v, u) >= lo && weightToKg(v, u) <= hi;

	if (!f.sex) errors.sex = 'Choose the formula.';
	if (f.birth_year == null || !Number.isInteger(f.birth_year) || f.birth_year < 1900 || f.birth_year > thisYear)
		errors.birth_year = `Enter a year from 1900 to ${thisYear}.`;
	const cm = heightCm(f);
	if (cm == null || cm < 50 || cm > 272)
		errors.height = f.height_unit === 'cm' ? 'Enter a height from 50 to 272 cm.' : 'Enter a height from 1′ 8″ to 8′ 11″.';
	if (!inKg(f.start_weight, 20, 400)) errors.start_weight = `Enter a weight from ${w(20)} to ${w(400)}.`;
	if (f.goal_weight != null && !inKg(f.goal_weight, 20, 400))
		errors.goal_weight = `Enter a goal from ${w(20)} to ${w(400)}, or leave it empty.`;
	if (f.pace == null || f.pace < 0 || toKg(f.pace, u)! > 1)
		errors.pace = `Enter a pace from 0 to ${u === 'kg' ? '1 kg' : '2.2 lb'} a week.`;
	if (f.typical_steps == null || !Number.isInteger(f.typical_steps) || f.typical_steps < 0 || f.typical_steps > 150_000)
		errors.typical_steps = 'Enter whole steps from 0 to 150,000.';
	if (Object.keys(errors).length) return { errors };

	return {
		errors,
		value: {
			timezone: f.timezone,
			weight_unit: u,
			height_unit: f.height_unit,
			sex: f.sex as Sex,
			birth_year: f.birth_year!,
			height_cm: cm!,
			start_weight_kg: toKg(f.start_weight, u)!,
			goal_kg: toKg(f.goal_weight, u),
			pace_kg_per_week: Math.min(1, toKg(f.pace, u)!),
			typical_steps: f.typical_steps!
		}
	};
}
