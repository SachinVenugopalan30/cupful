// SPDX-License-Identifier: AGPL-3.0-or-later
// Transport and server failures: a whole-batch rejection resends per op, a 401 pauses without a
// redirect, and a 5xx backs off without losing the ops.

import { describe, expect, it, vi } from 'vitest';
import { countFor, enqueue, flush } from './queue.ts';
import {
	authState,
	boot,
	calls,
	deleteOp,
	installQueueTestHooks,
	json,
	newUser,
	stubFetch,
} from './queue-test-helpers.ts';
import { connection, queueState } from './state.svelte.ts';
import type { Op } from '../sync/index.ts';

installQueueTestHooks();

describe('failure handling', () => {
	it('resends a rejected batch one op at a time and fails only the bad op', async () => {
		const userId = newUser();
		await boot(userId);
		const good = deleteOp();
		const bad = deleteOp();
		await enqueue([good, bad], userId);
		stubFetch(async (url, init) => {
			if (url === '/api/auth/state') return authState(userId);
			const ops = (JSON.parse(init?.body as string) as { ops: Op[] }).ops;
			if (ops.length > 1) return json({ error: 'the batch is bad' }, 400);
			return ops[0].op_id === bad.op_id
				? json({ error: 'this one is bad' }, 400)
				: json({ results: [{ op_id: ops[0].op_id, status: 'applied' }] });
		});

		const results = await flush(userId);

		expect(results.get(good.op_id)?.status).toBe('applied');
		expect(await countFor(userId)).toBe(1);
		expect(queueState.ops[0].op_id).toBe(bad.op_id);
		expect(queueState.ops[0].status).toBe('failed');
		expect(queueState.ops[0].error).toBe('The server answered with an error (400).');
	});

	it('pauses auth on a 401, keeps the ops and does not redirect', async () => {
		const userId = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		const assign = vi.fn();
		vi.stubGlobal('location', { assign });
		stubFetch(async (url) => {
			if (url === '/api/auth/state') return authState(userId);
			return json({ error: 'no session' }, 401);
		});

		await flush(userId);

		expect(connection.paused).toBe('auth');
		expect(assign).not.toHaveBeenCalled();
		expect(await countFor(userId)).toBe(1);
	});

	it('backs off on a 5xx and keeps the ops pending', async () => {
		const userId = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		const fetch = stubFetch(async (url) => {
			if (url === '/api/auth/state') return authState(userId);
			return json({ error: 'boom' }, 503);
		});

		await flush(userId);

		expect(connection.paused).toBe('offline');
		expect(await countFor(userId)).toBe(1);
		expect(queueState.ops[0].status).toBe('pending');
		await flush(userId);
		expect(calls(fetch, '/api/sync')).toBe(1);
	});
});
