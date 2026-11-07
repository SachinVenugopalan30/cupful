// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { markOffline, markOnline, onReachable, probeNow } from './connectivity.ts';
import { connection } from './state.svelte.ts';

const ok = () => new Response('{}', { status: 200 });

beforeEach(() => {
	connection.online = true;
	connection.confirmedOffline = false;
	connection.paused = null;
});

afterEach(() => {
	vi.unstubAllGlobals();
	connection.online = true;
	connection.confirmedOffline = false;
	connection.paused = null;
});

describe('markOnline', () => {
	it('wakes the queue once after an offline transition', () => {
		const woken = vi.fn();
		const off = onReachable(woken);
		connection.online = false;

		markOnline();

		expect(connection.online).toBe(true);
		expect(connection.confirmedOffline).toBe(false);
		expect(woken).toHaveBeenCalledOnce();

		markOnline();
		expect(woken).toHaveBeenCalledOnce();
		off();
	});

	it('clears an offline pause but leaves a sign-in pause alone', () => {
		connection.online = false;
		connection.paused = 'offline';
		markOnline();
		expect(connection.paused).toBeNull();

		connection.online = false;
		connection.paused = 'auth';
		markOnline();
		expect(connection.paused).toBe('auth');
	});
});

describe('markOffline', () => {
	it('keeps a stronger pause than offline', () => {
		connection.paused = 'auth';

		markOffline();

		expect(connection.online).toBe(false);
		expect(connection.paused).toBe('auth');
	});

	it('pauses the queue when nothing else is paused', () => {
		markOffline();

		expect(connection.online).toBe(false);
		expect(connection.confirmedOffline).toBe(false);
		expect(connection.paused).toBe('offline');
	});
});

describe('probeNow', () => {
	it('comes back online when the server answers', async () => {
		connection.online = false;
		const fetch = vi.fn(async () => ok());
		vi.stubGlobal('fetch', fetch);

		expect(await probeNow()).toBe(true);
		expect(connection.online).toBe(true);
		expect(fetch).toHaveBeenCalledWith(
			'/api/health',
			expect.objectContaining({ cache: 'no-store' })
		);
	});

	it('stays offline when the request cannot reach the server', async () => {
		connection.online = false;
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);

		expect(await probeNow()).toBe(false);
		expect(connection.online).toBe(false);
		expect(connection.confirmedOffline).toBe(true);
	});
});
