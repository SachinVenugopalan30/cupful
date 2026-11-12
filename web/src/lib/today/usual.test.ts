// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { HistoryMeal } from '#lib/api.ts';
import {
	USUAL_MIN_TIMES,
	dismissUsual,
	isDismissed,
	pickUsual,
	readDismissed,
	usualAlternatives,
	usualKey,
	usualLogOp,
	usualMeal,
	usualUndoOp
} from './usual.ts';

const today = '2026-11-12';

function meal(overrides: Partial<HistoryMeal> = {}): HistoryMeal {
	const id = overrides.id ?? 'm1';
	return {
		id,
		date: '2026-11-10',
		meal_type: 'lunch',
		description: 'Oats',
		label: 'Oats',
		scale: 1,
		assumptions: null,
		confidence: 'medium',
		source: 'manual',
		provider_id: null,
		updated_at: 1,
		times: USUAL_MIN_TIMES,
		last_date: '2026-11-10',
		items: [
			{
				id: `i-${id}`,
				name: 'Oats',
				portion_text: '1 bowl',
				grams: 100,
				kcal: 350,
				protein_g: 12,
				carbs_g: 60,
				fat_g: 7,
				food_id: null,
				origin: 'ai_estimate'
			}
		],
		totals: { kcal: 350, protein_g: 12, carbs_g: 60, fat_g: 7 },
		...overrides
	};
}

describe('usualKey', () => {
	it('keys on the slot and the normalized label', () => {
		expect(usualKey('lunch', '  Oats   With Banana ')).toBe('lunch:oats with banana');
	});
});

describe('pickUsual', () => {
	it('takes the top usual for the current slot', () => {
		const lunch = meal({ id: 'lunch' });
		const breakfast = meal({ id: 'breakfast', meal_type: 'breakfast' });
		expect(pickUsual([breakfast, lunch], 'lunch', today)?.id).toBe('lunch');
	});

	it('needs at least three past meals', () => {
		expect(pickUsual([meal({ times: 2 })], 'lunch', today)).toBeNull();
		expect(pickUsual([meal({ times: 3 })], 'lunch', today)).not.toBeNull();
	});

	it('hides the top usual after it was logged today', () => {
		expect(pickUsual([meal({ last_date: today })], 'lunch', today)).toBeNull();
	});

	it('skips dismissed usuals and offers the next one', () => {
		const dismissed = [usualKey('lunch', 'Oats')];
		const next = meal({ id: 'next', label: 'Rice bowl', description: 'Rice bowl' });
		expect(pickUsual([meal(), next], 'lunch', today, dismissed)?.id).toBe('next');
	});
});

describe('usualAlternatives', () => {
	it('lists the other close groups from the same slot and excludes the shown one', () => {
		const top = meal({ id: 'a', label: 'Oats', description: 'Oats' });
		const second = meal({ id: 'b', label: 'Rice bowl', description: 'Rice bowl' });
		const third = meal({ id: 'c', label: 'Soup', description: 'Soup' });
		const otherSlot = meal({ id: 'd', meal_type: 'dinner', label: 'Pasta' });
		const out = usualAlternatives([top, second, third, otherSlot], 'lunch', top, []);
		expect(out.map((m) => m.id)).toEqual(['b', 'c']);
	});

	it('drops dismissed groups and keeps at most two', () => {
		const top = meal({ id: 'a', label: 'Oats', description: 'Oats' });
		const out = usualAlternatives(
			[
				top,
				meal({ id: 'b', label: 'Rice bowl', description: 'Rice bowl' }),
				meal({ id: 'c', label: 'Soup', description: 'Soup' }),
				meal({ id: 'd', label: 'Pasta', description: 'Pasta' })
			],
			'lunch',
			top,
			[usualKey('lunch', 'Rice bowl')]
		);
		expect(out.map((m) => m.label)).toEqual(['Soup', 'Pasta']);
	});
});

describe('local dismissal', () => {
	function store(seed?: string) {
		let value = seed ?? null;
		return {
			getItem: () => value,
			setItem: (_key: string, next: string) => (value = next)
		};
	}

	it('reads an empty or broken list safely', () => {
		expect(readDismissed(store())).toEqual([]);
		expect(readDismissed(store('{oops'))).toEqual([]);
	});

	it('adds a dismissal once', () => {
		const s = store();
		dismissUsual('lunch', 'Oats', s);
		const list = dismissUsual('lunch', 'oats', s);
		expect(list).toEqual(['lunch:oats']);
		expect(isDismissed(meal(), list)).toBe(true);
	});
});

describe('usual write payloads', () => {
	it('logs a fresh repeat for the target date', () => {
		const usual = meal();
		const op = usualLogOp(usual, today);
		const payload = op.kind === 'meal.upsert' ? op.payload : null;
		expect(payload).toMatchObject({
			date: today,
			meal_type: 'lunch',
			description: 'Oats',
			source: 'repeat'
		});
		expect(payload!.id).not.toBe(usual.id);
		expect(payload!.items[0].id).not.toBe(usual.items[0].id);
		expect(usualMeal(usual, today).items[0].kcal).toBe(350);
	});

	it('undoes the new meal by id', () => {
		const usual = meal();
		const logged = usualMeal(usual, today);
		const undo = usualUndoOp(logged.id);
		expect(undo.kind).toBe('meal.delete');
		expect(undo.payload).toEqual({ id: logged.id });
	});
});
