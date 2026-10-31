// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Day, Workout } from '../api';
import {
	formatClock,
	formatDuration,
	formatKm,
	groupByDate,
	gymDates,
	lastTrained,
	parseCount,
	parseWeight,
	sessionsIn,
	sparklinePoints,
	stepsAverage,
	stepsVsTypical,
	toggle,
	weekWindow,
	weightDelta,
	workoutKind
} from './logic';

const day = (date: string, o: Partial<Day> = {}): Day => ({
	date,
	steps: null,
	steps_source: null,
	gym_minutes: null,
	gym_source: null,
	trained: [],
	weight_kg: null,
	...o
});
const wo = (date: string, o: Partial<Workout> = {}): Workout => ({
	id: 1,
	date,
	start: `${date}T07:00:00Z`,
	end: `${date}T07:30:00Z`,
	type: 'RUNNING',
	type_label: 'Running',
	name: null,
	active_minutes: 30,
	kcal: null,
	distance_m: null,
	avg_hr: null,
	counts_as_gym: false,
	...o
});

describe('parseCount', () => {
	it('empty clears the field', () => {
		expect(parseCount('', 150000, 'Steps')).toEqual({ value: null });
		expect(parseCount('  ', 150000, 'Steps')).toEqual({ value: null });
	});
	it('accepts whole numbers in range, including 0', () => {
		expect(parseCount('8000', 150000, 'Steps')).toEqual({ value: 8000 });
		expect(parseCount('0', 600, 'Gym minutes')).toEqual({ value: 0 });
	});
	it('rejects fractions, negatives, and values over the max, naming the field', () => {
		expect(parseCount('12.5', 600, 'Gym minutes')).toHaveProperty('error');
		expect(parseCount('-1', 600, 'Gym minutes')).toHaveProperty('error');
		expect(parseCount('700', 600, 'Gym minutes')).toEqual({ error: 'Gym minutes must be a whole number from 0 to 600.' });
	});
});

describe('parseWeight', () => {
	it('converts to kg from the display unit', () => {
		expect(parseWeight('80', 'kg')).toEqual({ value: 80 });
		const lb = parseWeight('176', 'lb');
		expect((lb as { value: number }).value).toBeCloseTo(79.83, 1);
	});
	it('clears when empty and rejects out of range (server: 20 to 400 kg)', () => {
		expect(parseWeight('', 'kg')).toEqual({ value: null });
		expect(parseWeight('10', 'kg')).toHaveProperty('error');
		expect(parseWeight('900', 'lb')).toHaveProperty('error');
		expect(parseWeight('abc', 'kg')).toHaveProperty('error');
	});
});

describe('toggle', () => {
	it('adds in canonical order and removes', () => {
		expect(toggle(['Legs'], 'Chest')).toEqual(['Chest', 'Legs']);
		expect(toggle(['Chest', 'Legs'], 'Chest')).toEqual(['Legs']);
	});
});

describe('week and insights', () => {
	it('weekWindow ends on the date, across month and year ends', () => {
		expect(weekWindow('2026-10-30')).toEqual(['2026-10-24', '2026-10-25', '2026-10-26', '2026-10-27', '2026-10-28', '2026-10-29', '2026-10-30']);
		expect(weekWindow('2026-01-03')[0]).toBe('2025-12-28');
	});
	it('stepsVsTypical is a whole percent, null without both', () => {
		expect(stepsVsTypical(8300, 10000)).toBe(83);
		expect(stepsVsTypical(12000, 10000)).toBe(120);
		expect(stepsVsTypical(null, 10000)).toBeNull();
		expect(stepsVsTypical(5000, null)).toBeNull();
		expect(stepsVsTypical(5000, 0)).toBeNull();
	});
	it('stepsAverage skips unlogged days', () => {
		const days = [day('2026-10-28', { steps: 6000 }), day('2026-10-29'), day('2026-10-30', { steps: 8000 }), day('2026-10-01', { steps: 99999 })];
		expect(stepsAverage(days, weekWindow('2026-10-30'))).toBe(7000);
		expect(stepsAverage([], weekWindow('2026-10-30'))).toBeNull();
	});
	it('gym days come from minutes or gym-counted workouts; sessions and last trained follow', () => {
		const days = [day('2026-10-26', { gym_minutes: 45 }), day('2026-10-27', { gym_minutes: 0 })];
		const ws = [wo('2026-10-29', { counts_as_gym: true }), wo('2026-10-28', { counts_as_gym: false })];
		const gym = gymDates(days, ws);
		expect([...gym].sort()).toEqual(['2026-10-26', '2026-10-29']);
		expect(sessionsIn(gym, weekWindow('2026-10-30'))).toBe(2);
		expect(sessionsIn(gym, weekWindow('2026-10-27'))).toBe(1);
		expect(lastTrained(gym, '2026-10-30')).toBe('2026-10-29');
		expect(lastTrained(gym, '2026-10-28')).toBe('2026-10-26');
		expect(lastTrained(gym, '2026-10-20')).toBeNull();
	});
	it('weightDelta compares the last two weigh-ins up to the date', () => {
		const days = [day('2026-10-10', { weight_kg: 81 }), day('2026-10-20', { weight_kg: 80.4 }), day('2026-10-29', { weight_kg: 80 })];
		const d = weightDelta(days, '2026-10-30')!;
		expect(d.deltaKg).toBeCloseTo(-0.4);
		expect(d.prevDate).toBe('2026-10-20');
		expect(weightDelta(days, '2026-10-25')!.deltaKg).toBeCloseTo(-0.6);
		expect(weightDelta(days, '2026-10-15')).toBeNull();
	});
	it('groupByDate keeps order within a day', () => {
		const m = groupByDate([wo('2026-10-29', { id: 1 }), wo('2026-10-30', { id: 2 }), wo('2026-10-29', { id: 3 })]);
		expect(m.get('2026-10-29')!.map((w) => w.id)).toEqual([1, 3]);
		expect(m.get('2026-10-28')).toBeUndefined();
	});
});

describe('sparklinePoints', () => {
	it('maps low to the bottom and high to the top, joining across gaps', () => {
		expect(sparklinePoints([1, null, 3], 100, 20, 0)).toBe('0,20 100,0');
	});
	it('needs two points; a flat series sits mid-height', () => {
		expect(sparklinePoints([5, null, null], 100, 20)).toBe('');
		expect(sparklinePoints([5, 5], 100, 20, 0)).toBe('0,10 100,10');
	});
});

describe('workout formatting', () => {
	it('km, one decimal', () => {
		expect(formatKm(5200)).toBe('5.2 km');
		expect(formatKm(10000)).toBe('10 km');
	});
	it('duration', () => {
		expect(formatDuration(31)).toBe('31 min');
		expect(formatDuration(65)).toBe('1 h 05 min');
	});
	it('clock in the given zone', () => {
		expect(formatClock('2026-10-30T07:42:00Z', 'UTC').toLowerCase()).toBe('7:42 am');
	});
	it('kind from the Google type', () => {
		expect(workoutKind('RUNNING')).toBe('run');
		expect(workoutKind('STRENGTH_TRAINING')).toBe('strength');
		expect(workoutKind('BIKING')).toBe('bike');
		expect(workoutKind('YOGA')).toBe('other');
	});
});
