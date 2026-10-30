// SPDX-License-Identifier: AGPL-3.0-or-later
// The identity guard: before a flush the queue asks who the session belongs to and refuses to
// send another account's ops, pauses for sign-in on an expired session, and backs off when the
// guard itself cannot reach the server.

import { describe, expect, it } from 'vitest';
import { countFor, enqueue, flush } from './queue.ts';
import {
	authState,
	boot,
	calls,
	deleteOp,
	installQueueTestHooks,
	newUser,
	stubFetch,
} from './queue-test-helpers.ts';
import { connection, queueState } from './state.svelte.ts';

installQueueTestHooks();

describe('identity guard', () => {
	it('posts nothing when the session is another user, keeping every op', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = [deleteOp()];
		await enqueue(ops, userId);
		const fetch = stubFetch(async (url) => {
			if (url === '/api/auth/state') return authState(userId + 1);
			throw new Error('must not post');
		});

		const results = await flush(userId);

		expect(calls(fetch, '/api/sync')).toBe(0);
		expect(results.size).toBe(0);
		expect(connection.paused).toBe('identity');
		expect(await countFor(userId)).toBe(1);
		expect(queueState.ops[0].status).toBe('pending');
	});

	it('pauses for sign-in when the session expired', async () => {
		const userId = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		const fetch = stubFetch(async (url) => {
			if (url === '/api/auth/state') return authState(null);
			throw new Error('must not post');
		});

		await flush(userId);

		expect(calls(fetch, '/api/sync')).toBe(0);
		expect(connection.paused).toBe('auth');
		expect(await countFor(userId)).toBe(1);
	});

	it('backs off without an identity pause when the guard cannot reach the server', async () => {
		const userId = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		const fetch = stubFetch(async () => {
			throw new TypeError('Failed to fetch');
		});

		await flush(userId);

		expect(connection.paused).toBe('offline');
		expect(calls(fetch, '/api/auth/state')).toBe(1);
		expect(await countFor(userId)).toBe(1);
		// A pending backoff stops an immediate retry from hammering the server.
		await flush(userId);
		expect(calls(fetch, '/api/auth/state')).toBe(1);
	});
});
