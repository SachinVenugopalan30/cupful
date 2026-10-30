// SPDX-License-Identifier: AGPL-3.0-or-later
// Flush results and per-op recovery: what a landed answer does to the queue, how retry and
// discard act on kept ops, and how clearUser drops one account's queue and cache.

import { describe, expect, it } from 'vitest';
import { getCachedBody, putCachedBody } from './cache.ts';
import { clearUser, countFor, discard, enqueue, flush, retry } from './queue.ts';
import {
	applied,
	boot,
	day,
	deleteOp,
	installQueueTestHooks,
	newUser,
	serving,
} from './queue-test-helpers.ts';
import { connection, queueState } from './state.svelte.ts';

installQueueTestHooks();

describe('flush results', () => {
	it('removes applied and duplicate ops', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = [deleteOp(), deleteOp()];
		await enqueue(ops, userId);
		serving(userId, (sent) => [
			{ op_id: sent[0].op_id, status: 'applied' },
			{ op_id: sent[1].op_id, status: 'duplicate' },
		]);

		const results = await flush(userId);

		expect(results.get(ops[0].op_id)?.status).toBe('applied');
		expect(results.get(ops[1].op_id)?.status).toBe('duplicate');
		expect(await countFor(userId)).toBe(0);
		expect(queueState.ops).toEqual([]);
	});

	it('keeps stale and rejected ops with their reason and carries on', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = [deleteOp(), deleteOp(), deleteOp()];
		await enqueue(ops, userId);
		serving(userId, (sent) => [
			{ op_id: sent[0].op_id, status: 'stale' },
			{ op_id: sent[1].op_id, status: 'rejected', error: 'Bad op.' },
			{ op_id: sent[2].op_id, status: 'applied' },
		]);

		const results = await flush(userId);

		expect(results.size).toBe(3);
		expect(results.get(ops[2].op_id)?.status).toBe('applied');
		const kept = await countFor(userId);
		expect(kept).toBe(2);
		const byId = new Map(queueState.ops.map((o) => [o.op_id, o]));
		expect(byId.get(ops[0].op_id)?.status).toBe('stale');
		expect(byId.get(ops[0].op_id)?.error).toBe('A newer version replaced this change.');
		expect(byId.get(ops[1].op_id)?.status).toBe('rejected');
		expect(byId.get(ops[1].op_id)?.error).toBe('Bad op.');
		expect(connection.failed).toBe(2);
	});
});

describe('retry, discard and clear', () => {
	it('retries a stale op and discards the other one', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = [deleteOp(), deleteOp()];
		await enqueue(ops, userId);
		serving(userId, (sent) => [
			{ op_id: sent[0].op_id, status: 'stale' },
			{ op_id: sent[1].op_id, status: 'rejected', error: 'No.' },
		]);
		await flush(userId);

		const stale = queueState.ops.find((o) => o.status === 'stale')!;
		serving(userId, applied);
		await retry(stale.op_id);

		expect(await countFor(userId)).toBe(1);
		expect(queueState.ops[0].status).toBe('rejected');

		await discard(queueState.ops[0].op_id);
		expect(await countFor(userId)).toBe(0);
		expect(queueState.ops).toEqual([]);
	});

	it('clears one user’s queue and cache', async () => {
		const userId = newUser();
		const other = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		await enqueue([deleteOp()], other);
		await putCachedBody(userId, '/api/days/2026-10-29', day('2026-10-29', 1));

		await clearUser(userId);

		expect(await countFor(userId)).toBe(0);
		expect(await getCachedBody(userId, '/api/days/2026-10-29')).toBeUndefined();
		expect(await countFor(other)).toBe(1);
	});
});
