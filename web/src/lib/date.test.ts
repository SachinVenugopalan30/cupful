// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { addDays, todayIn } from './date';

describe('todayIn', () => {
	it('is the date in the given timezone', () => {
		const now = new Date('2026-10-28T20:30:00Z');
		expect(todayIn('Asia/Kolkata', now)).toBe('2026-10-29');
		expect(todayIn('America/Los_Angeles', now)).toBe('2026-10-28');
		expect(todayIn('UTC', now)).toBe('2026-10-28');
	});
});

describe('addDays', () => {
	it('crosses month, year and leap-day boundaries', () => {
		expect(addDays('2026-10-31', 1)).toBe('2026-11-01');
		expect(addDays('2027-01-01', -1)).toBe('2026-12-31');
		expect(addDays('2028-02-28', 1)).toBe('2028-02-29');
	});
	it('ignores DST changes', () => {
		expect(addDays('2026-03-08', 1)).toBe('2026-03-09');
	});
});
