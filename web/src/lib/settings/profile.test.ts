// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Profile } from '../api';
import { fromForm, setHeightUnit, setWeightUnit, toForm } from './profile';

const saved: Profile = {
	timezone: 'Asia/Kolkata',
	weight_unit: 'lb',
	height_unit: 'ftin',
	version: {
		effective_from: '2026-10-01',
		sex: 'female',
		birth_year: 1990,
		height_cm: 165,
		start_weight_kg: 70,
		goal_kg: 62,
		pace_kg_per_week: 0.5,
		typical_steps: 7000
	}
};

describe('profile form', () => {
	it('shows saved metric values in the chosen units and saves them back as metric', () => {
		const f = toForm(saved, 'UTC');
		expect(f).toMatchObject({ start_weight: 154.3, goal_weight: 136.7, pace: 1.1, height_ft: 5, height_in: 5 });
		const { value, errors } = fromForm(f, 2026);
		expect(errors).toEqual({});
		expect(value!.start_weight_kg).toBeCloseTo(70, 1);
		expect(value!.goal_kg).toBeCloseTo(62, 1);
		expect(value!.pace_kg_per_week).toBeCloseTo(0.5, 1);
		expect(value!.height_cm).toBeCloseTo(165, 0);
		expect(value).toMatchObject({ timezone: 'Asia/Kolkata', sex: 'female', weight_unit: 'lb', height_unit: 'ftin', typical_steps: 7000 });
	});

	it('starts a new profile in the browser timezone with empty fields', () => {
		const f = toForm({ timezone: 'UTC', weight_unit: 'kg', height_unit: 'cm', version: null }, 'Europe/Oslo');
		expect(f.timezone).toBe('UTC');
		expect(f.start_weight).toBeNull();
		expect(fromForm(f, 2026).errors).toMatchObject({ sex: expect.any(String), start_weight: expect.any(String) });
	});

	it('converts displayed values when the units change', () => {
		let f = toForm({ ...saved, weight_unit: 'kg', height_unit: 'cm' }, 'UTC');
		expect(f).toMatchObject({ start_weight: 70, pace: 0.5, height_cm: 165 });
		f = setHeightUnit(setWeightUnit(f, 'lb'), 'ftin');
		expect(f).toMatchObject({ weight_unit: 'lb', start_weight: 154.3, height_ft: 5, height_in: 5 });
		f = setHeightUnit(setWeightUnit(f, 'kg'), 'cm');
		expect(f).toMatchObject({ start_weight: 70, goal_weight: 62, pace: 0.5, height_cm: 165 });
	});

	it('validates like the server, in the user’s units, and leaves goal optional', () => {
		const f = { ...toForm(saved, 'UTC'), goal_weight: null };
		expect(fromForm(f, 2026).value!.goal_kg).toBeNull();
		const bad = fromForm(
			{ ...f, birth_year: 2027, start_weight: 900, pace: 2.3, typical_steps: 1.5, height_ft: 1, height_in: 0 },
			2026
		).errors;
		expect(Object.keys(bad).sort()).toEqual(['birth_year', 'height', 'pace', 'start_weight', 'typical_steps']);
		expect(bad.start_weight).toMatch(/lb/);
		expect(bad.pace).toMatch(/2\.2 lb/);
		expect(fromForm({ ...f, pace: 2.2 }, 2026).errors).toEqual({});
	});
});
