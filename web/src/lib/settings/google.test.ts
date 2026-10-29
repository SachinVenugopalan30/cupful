// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { googleResult, googleView } from './google.ts';

const connected = { configured: true, connected: true, last_sync_at: null, last_error: null } as const;

describe('googleView', () => {
	it('hides the card until the server is configured', () => {
		expect(googleView(null)).toBe('hidden');
		expect(googleView({ configured: false })).toBe('hidden');
	});
	it('maps the connection states', () => {
		expect(googleView({ configured: true, connected: false })).toBe('connect');
		expect(googleView({ ...connected, needs_reconnect: false })).toBe('connected');
		expect(googleView({ ...connected, needs_reconnect: true })).toBe('reconnect');
	});
});

describe('googleResult', () => {
	it('reads the redirect outcome', () => {
		expect(googleResult('?google=connected')?.ok).toBe(true);
		expect(googleResult('?google=denied')?.ok).toBe(false);
		expect(googleResult('?google=error')?.ok).toBe(false);
		expect(googleResult('')).toBeNull();
		expect(googleResult('?google=wat')).toBeNull();
	});
});
