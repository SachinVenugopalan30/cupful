// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it, vi } from 'vitest';
import type { DraftItem } from './meal';
import { aliasFailure, MACROS, rememberAliases } from './review';

const item = (over: Partial<DraftItem> = {}): DraftItem => ({
	id: 'item',
	name: 'Oats',
	phrase: 'oats',
	swapped: true,
	remember: true,
	portion_text: '1 bowl',
	grams: 50,
	kcal: 190,
	protein_g: 6,
	carbs_g: 33,
	fat_g: 3,
	food_id: 12,
	origin: 'food_db',
	editable: false,
	options: [],
	chosen: 0,
	...over
});

describe('MACROS', () => {
	it('lists the four editable numbers', () => {
		expect(MACROS.map(([key]) => key)).toEqual(['kcal', 'protein_g', 'carbs_g', 'fat_g']);
	});
});

describe('rememberAliases', () => {
	it('saves each flagged phrase trimmed, and clears its flag', async () => {
		const oats = item({ phrase: '  oats ' });
		const alias = vi.fn(async () => ({}));
		expect(await rememberAliases([oats], alias)).toEqual([]);
		expect(alias).toHaveBeenCalledWith('oats', 12);
		expect(oats.remember).toBe(false);
	});

	it('skips items without a flag, a food, or a phrase', async () => {
		const notFlagged = item({ remember: false });
		const noFood = item({ food_id: null });
		const noPhrase = item({ phrase: '  ' });
		const alias = vi.fn(async () => ({}));
		expect(await rememberAliases([notFlagged, noFood, noPhrase], alias)).toEqual([]);
		expect(alias).not.toHaveBeenCalled();
		expect(noFood.remember).toBe(true);
		expect(noPhrase.remember).toBe(true);
	});

	it('returns failed phrases and keeps their flags set', async () => {
		const ok = item({ id: 'ok', phrase: 'oats' });
		const bad = item({ id: 'bad', phrase: 'muesli' });
		const alias = vi.fn(async (phrase: string) => {
			if (phrase === 'muesli') throw new Error('nope');
			return {};
		});
		expect(await rememberAliases([ok, bad], alias)).toEqual(['muesli']);
		expect(ok.remember).toBe(false);
		expect(bad.remember).toBe(true);
	});
});

describe('aliasFailure', () => {
	it('names the phrases that failed, quoting each', () => {
		expect(aliasFailure(['oats', 'muesli'])).toBe(
			'The meal is saved, but I couldn\'t remember "oats", "muesli". Save again to retry, or close.'
		);
	});
});
