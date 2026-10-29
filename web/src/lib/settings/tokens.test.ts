// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { curlExample } from './tokens';

describe('curlExample', () => {
	it('posts JSON with the bearer token to the ingest endpoint', () => {
		const c = curlExample('https://cupful.example', 'cupful_abc', '2026-10-29');
		expect(c).toContain("-H 'Authorization: Bearer cupful_abc'");
		expect(c).toContain('https://cupful.example/api/ingest/activity');
		expect(c).toContain('"date":"2026-10-29"');
		expect(c).toContain("-H 'Content-Type: application/json'");
	});
	it('defaults the date to today', () => {
		expect(curlExample('http://x', 't')).toMatch(/"date":"\d{4}-\d{2}-\d{2}"/);
	});
});
