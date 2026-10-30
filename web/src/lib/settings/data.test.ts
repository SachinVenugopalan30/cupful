// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import {
	fallbackFilename,
	formatImportSummary,
	formatRestoreSummary,
	parseBackup,
	responseFilename
} from './data.ts';

describe('formatImportSummary', () => {
	it('names each kind of change and counts items and days', () => {
		expect(
			formatImportSummary({
				meals_created: 0,
				meals_updated: 1,
				meals_unchanged: 2,
				items: 5,
				days: 2
			})
		).toBe('Imported 3 meals (1 updated, 2 unchanged), 5 items, 2 days.');
	});

	it('uses singular nouns for one, and lists new meals first', () => {
		expect(
			formatImportSummary({
				meals_created: 1,
				meals_updated: 1,
				meals_unchanged: 0,
				items: 1,
				days: 1
			})
		).toBe('Imported 2 meals (1 new, 1 updated), 1 item, 1 day.');
	});

	it('handles a file that changes nothing', () => {
		expect(
			formatImportSummary({
				meals_created: 0,
				meals_updated: 0,
				meals_unchanged: 0,
				items: 0,
				days: 0
			})
		).toBe('Imported 0 meals, 0 items, 0 days.');
	});
});

describe('formatRestoreSummary', () => {
	it('counts every restored collection, pluralized', () => {
		expect(
			formatRestoreSummary({
				profile_versions: 2,
				days: 2,
				meals: 3,
				items: 5,
				foods: 4,
				aliases: 1,
				portions: 2,
				products: 0
			})
		).toBe('Restored 2 days, 3 meals, 5 items, 4 foods, 1 alias, 2 portions and 0 products.');
	});
});

describe('responseFilename', () => {
	it('reads a quoted filename', () => {
		expect(responseFilename('attachment; filename="cupful-2026-10-29.csv"')).toBe(
			'cupful-2026-10-29.csv'
		);
	});

	it('reads an unquoted filename', () => {
		expect(responseFilename('attachment; filename=cupful.csv')).toBe('cupful.csv');
	});

	it('reads an RFC 5987 filename and decodes it', () => {
		expect(responseFilename("attachment; filename*=UTF-8''cupful%20backup.json")).toBe(
			'cupful backup.json'
		);
	});

	it('drops directory parts', () => {
		expect(responseFilename('attachment; filename="../../etc/passwd"')).toBe('passwd');
	});

	it('returns null when the header is missing or names nothing', () => {
		expect(responseFilename(null)).toBeNull();
		expect(responseFilename('attachment')).toBeNull();
	});
});

describe('fallbackFilename', () => {
	it('names the file from the path and the local date', () => {
		expect(fallbackFilename('/api/export.csv', '2026-10-29')).toBe('cupful-2026-10-29.csv');
		expect(fallbackFilename('/api/backup', '2026-10-29')).toBe('cupful-backup-2026-10-29.json');
	});
});

describe('parseBackup', () => {
	it('returns a JSON object', () => {
		expect(parseBackup('{"meals":[]}')).toEqual({ meals: [] });
	});

	it('rejects text that is not JSON', () => {
		expect(() => parseBackup('<html>')).toThrow(/isn't valid JSON/);
	});

	it('rejects JSON that is not a backup document', () => {
		expect(() => parseBackup('[]')).toThrow(/isn't a Cupful backup/);
		expect(() => parseBackup('null')).toThrow(/isn't a Cupful backup/);
	});
});
