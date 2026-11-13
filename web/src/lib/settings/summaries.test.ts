// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { googleSummary, syncSummary } from './summaries';

describe('syncSummary', () => {
	const op = (status: string) => ({ status });
	it('says everything is synced when the queue is empty', () => {
		expect(syncSummary([])).toBe('All changes synced.');
	});
	it('counts changes still on their way', () => {
		expect(syncSummary([op('pending')])).toBe('1 change waiting.');
		expect(syncSummary([op('pending'), op('sending'), op('pending')])).toBe('3 changes waiting.');
	});
	it('puts changes that need a decision first, like the top bar does', () => {
		expect(syncSummary([op('rejected'), op('stale'), op('pending')])).toBe('2 changes need attention.');
		expect(syncSummary([op('failed')])).toBe('1 change needs attention.');
	});
});

describe('googleSummary', () => {
	const connected = {
		configured: true as const,
		connected: true as const,
		needs_reconnect: false,
		last_error: null,
		gym_types: [],
		exercise_types: []
	};
	const at = (iso: string) => `at ${iso}`;

	it('falls back to the description until the status loads', () => {
		expect(googleSummary(null, at)).toBeUndefined();
		expect(googleSummary({ configured: false }, at)).toBeUndefined();
	});
	it('says when it last synced', () => {
		expect(googleSummary({ ...connected, last_sync_at: '2026-11-13T12:10:00Z' }, at)).toBe(
			'Connected · synced at 2026-11-13T12:10:00Z'
		);
		expect(googleSummary({ ...connected, last_sync_at: null }, at)).toBe('Connected · not synced yet');
	});
	it('flags a connection that needs reconnecting, and one not made', () => {
		expect(googleSummary({ ...connected, needs_reconnect: true, last_sync_at: null }, at)).toBe(
			'Needs reconnecting'
		);
		expect(googleSummary({ configured: true, connected: false }, at)).toBe('Not connected');
	});
});
