// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { dayReadDate, keysToEvict } from './evict.ts';

const day = (n: number) => `2026-01-${String(n).padStart(2, '0')}`;
const entry = (path: string) => ({ key: `1:${path}`, path });
const dayEntries = (n: number) =>
	Array.from({ length: n }, (_, i) => day(i + 1)).flatMap((d) => [
		entry(`/api/meals?date=${d}`),
		entry(`/api/days/${d}`),
		entry(`/api/budget/${d}`)
	]);

describe('dayReadDate', () => {
	it('reads the date of day reads only', () => {
		expect(dayReadDate('/api/meals?date=2026-01-05')).toBe('2026-01-05');
		expect(dayReadDate('/api/days/2026-01-05')).toBe('2026-01-05');
		expect(dayReadDate('/api/budget/2026-01-05')).toBe('2026-01-05');
		expect(dayReadDate('/api/days?from=2026-01-01&to=2026-01-05')).toBeNull();
		expect(dayReadDate('/api/profile')).toBeNull();
	});
});

describe('keysToEvict', () => {
	it('keeps everything up to the limit', () => {
		expect(keysToEvict(dayEntries(5), new Set(), 5)).toEqual([]);
	});

	it('drops all reads of the oldest dates beyond the limit', () => {
		const out = keysToEvict(dayEntries(5), new Set(), 3);
		expect(out.sort()).toEqual(
			[day(1), day(2)]
				.flatMap((d) => [`/api/meals?date=${d}`, `/api/days/${d}`, `/api/budget/${d}`])
				.map((p) => `1:${p}`)
				.sort()
		);
	});

	it('never drops a protected date', () => {
		const out = keysToEvict(dayEntries(5), new Set([day(1)]), 3);
		expect(out.some((k) => k.includes(day(1)))).toBe(false);
		expect(out.some((k) => k.includes(day(2)))).toBe(true);
	});

	it('never selects other entries', () => {
		const rest = [entry('/api/profile'), entry('/api/trends?from=a&to=b'), entry('/api/products')];
		expect(keysToEvict([...rest, ...dayEntries(4)], new Set(), 1)).not.toContain('1:/api/profile');
	});

	it('defaults to 60 dates', () => {
		expect(keysToEvict(dayEntries(28), new Set())).toEqual([]);
		const many = Array.from({ length: 61 }, (_, i) => entry(`/api/days/2026-${String(Math.floor(i / 28) + 1).padStart(2, '0')}-${String((i % 28) + 1).padStart(2, '0')}`));
		expect(keysToEvict(many, new Set())).toEqual([many[0].key]);
	});
});
