// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Day, Workout } from '../api';
import {
	addedToBudget,
	weekBars,
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
	weekEndFor,
	weekDayLines,
	windowCovers,
	weightDelta,
	lastWeight,
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

	it('lastWeight returns the latest weigh-in on or before the date', () => {
		const days = [day('2026-10-10', { weight_kg: 81 }), day('2026-10-20', { weight_kg: 80.4 })];
		expect(lastWeight(days, '2026-10-25')).toEqual({ kg: 80.4, date: '2026-10-20' });
		expect(lastWeight(days, '2026-10-15')).toEqual({ kg: 81, date: '2026-10-10' });
		expect(lastWeight(days, '2026-10-05')).toBeNull();
	});

	it('windowCovers includes empty days inside the fetched range', () => {
		const win = { date: '2026-10-31', days: [], workouts: [] };
		expect(windowCovers(win, '2026-10-31', 30)).toBe(true);
		expect(windowCovers(win, '2026-10-02', 30)).toBe(true);
		expect(windowCovers(win, '2026-10-01', 30)).toBe(false);
		expect(windowCovers(win, '2026-10-25', 7)).toBe(true);
		expect(windowCovers(win, '2026-10-24', 7)).toBe(false);
	});

	it('weekEndFor keeps the window until the day leaves it', () => {
		expect(weekEndFor('2026-11-02', '2026-11-02')).toBe('2026-11-02');
		expect(weekEndFor('2026-11-01', '2026-11-02')).toBe('2026-11-02');
		expect(weekEndFor('2026-10-27', '2026-11-02')).toBe('2026-11-02');
		expect(weekEndFor('2026-10-26', '2026-11-02')).toBe('2026-10-26');
		expect(weekEndFor('2026-11-03', '2026-11-02')).toBe('2026-11-03');
	});
	it('groupByDate keeps order within a day', () => {
		const m = groupByDate([wo('2026-10-29', { id: 1 }), wo('2026-10-30', { id: 2 }), wo('2026-10-29', { id: 3 })]);
		expect(m.get('2026-10-29')!.map((w) => w.id)).toEqual([1, 3]);
		expect(m.get('2026-10-28')).toBeUndefined();
	});
});

describe('weekDayLines', () => {
	it('shows steps first and says when nothing else is logged', () => {
		expect(weekDayLines({ steps: 9100, gym_minutes: null, trained: [], workouts: [] })).toEqual([
			'9,100 steps',
			'No gym or workouts logged'
		]);
		expect(weekDayLines({ steps: null, gym_minutes: null, trained: [], workouts: [] })).toEqual([
			'No steps logged',
			'No gym or workouts logged'
		]);
	});

	it('adds gym minutes, trained parts and workout names', () => {
		expect(
			weekDayLines({
				steps: 9100,
				gym_minutes: 40,
				trained: ['Chest', 'Arms'],
				workouts: ['Strength training']
			})
		).toEqual(['9,100 steps', '40 min gym', 'Trained: Chest, Arms', 'Workout: Strength training']);
	});

	it('summarises more than two workouts', () => {
		expect(weekDayLines({ steps: 5000, gym_minutes: null, trained: [], workouts: ['Run', 'Walk', 'Yoga', 'Bike'] })).toEqual([
			'5,000 steps',
			'Workouts: Run, Walk +2'
		]);
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

describe('addedToBudget', () => {
	it('adds the two server values for display', () => {
		expect(addedToBudget({ steps_kcal: 84, gym_kcal: 100 })).toEqual({ steps: 84, gym: 100, total: 184 });
		expect(addedToBudget({ steps_kcal: 0, gym_kcal: 0 }).total).toBe(0);
	});
});

describe('weekBars', () => {
	const rows = [
		{ date: '2026-11-09', steps: 5000 },
		{ date: '2026-11-10', steps: null },
		{ date: '2026-11-11', steps: 10000 }
	];
	it('scales to the highest of the week and the typical day', () => {
		const w = weekBars(rows, 8000);
		expect(w.max).toBe(10000);
		expect(w.bars.map((b) => b.h)).toEqual([0.5, null, 1]);
		expect(w.typicalAt).toBe(0.8);
	});
	it('lets a high typical day set the scale, and skips the line without one', () => {
		expect(weekBars(rows, 20000).typicalAt).toBe(1);
		expect(weekBars(rows, 20000).bars[2].h).toBe(0.5);
		expect(weekBars(rows, null).typicalAt).toBeNull();
		expect(weekBars([{ date: 'd', steps: null }], null).max).toBe(1);
	});
});
