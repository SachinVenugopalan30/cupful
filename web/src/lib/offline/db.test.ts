// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import {
	allCacheForUser,
	allQueueForUser,
	clearUserData,
	deleteMeta,
	deleteQueueItem,
	getCacheEntry,
	getMeta,
	putCacheEntry,
	putMeta,
	putQueueItem
} from './db.ts';
import type { QueuedOp } from './types.ts';

let seq = 0;
function opId(): string {
	seq++;
	return `op-${seq}`;
}

function queued(user_id: number, queued_at = Date.now(), id = opId()): QueuedOp {
	return {
		op_id: id,
		user_id,
		op: { op_id: id, kind: 'meal.delete', at: '2026-10-29T12:00:00.000Z', payload: { id: 'm1' } },
		status: 'pending',
		queued_at,
		attempts: 0
	};
}

describe('offline db', () => {
	it('puts and deletes a queue item', async () => {
		const item = queued(9001);
		await putQueueItem(item);
		expect(await allQueueForUser(9001)).toEqual([item]);
		await deleteQueueItem(item.op_id);
		expect(await allQueueForUser(9001)).toEqual([]);
	});

	it('lists one user’s queue oldest first and keeps users apart', async () => {
		const a1 = queued(9002, 20);
		const a2 = queued(9002, 10);
		const b1 = queued(9003, 5);
		await putQueueItem(a1);
		await putQueueItem(a2);
		await putQueueItem(b1);

		expect((await allQueueForUser(9002)).map((q) => q.op_id)).toEqual([a2.op_id, a1.op_id]);
		expect((await allQueueForUser(9003)).map((q) => q.op_id)).toEqual([b1.op_id]);
	});

	it('stores cache entries per user without collisions', async () => {
		await putCacheEntry({
			key: '9004:/api/days/2026-10-29',
			user_id: 9004,
			path: '/api/days/2026-10-29',
			body: { date: '2026-10-29', steps: 100 },
			saved_at: 1
		});
		await putCacheEntry({
			key: '9005:/api/days/2026-10-29',
			user_id: 9005,
			path: '/api/days/2026-10-29',
			body: { date: '2026-10-29', steps: 200 },
			saved_at: 2
		});

		expect((await getCacheEntry(9004, '/api/days/2026-10-29'))?.body).toEqual({
			date: '2026-10-29',
			steps: 100
		});
		expect((await getCacheEntry(9005, '/api/days/2026-10-29'))?.body).toEqual({
			date: '2026-10-29',
			steps: 200
		});
	});

	it('clears only the given user’s queue and cache', async () => {
		const mine = queued(9006);
		const theirs = queued(9007);
		await putQueueItem(mine);
		await putQueueItem(theirs);
		await putCacheEntry({
			key: '9006:/api/profile',
			user_id: 9006,
			path: '/api/profile',
			body: { timezone: 'UTC' },
			saved_at: 1
		});
		await putCacheEntry({
			key: '9007:/api/profile',
			user_id: 9007,
			path: '/api/profile',
			body: { timezone: 'UTC' },
			saved_at: 1
		});

		await clearUserData(9006);

		expect(await allQueueForUser(9006)).toEqual([]);
		expect(await getCacheEntry(9006, '/api/profile')).toBeUndefined();
		expect((await allQueueForUser(9007)).map((q) => q.op_id)).toEqual([theirs.op_id]);
		expect(await getCacheEntry(9007, '/api/profile')).toBeDefined();
	});

	it('stores and deletes metadata', async () => {
		await putMeta('hello', { n: 1 });
		expect(await getMeta('hello')).toEqual({ n: 1 });
		await deleteMeta('hello');
		expect(await getMeta('hello')).toBeUndefined();
	});

	it('keeps the 60 newest dates and every date the queue references', async () => {
		const user = 9008;
		const date = (i: number) => new Date(Date.UTC(2026, 0, 1 + i)).toISOString().slice(0, 10);
		const put = (path: string) =>
			putCacheEntry({ key: `${user}:${path}`, user_id: user, path, body: {}, saved_at: 1 });
		await put('/api/profile');
		const held = queued(user);
		held.op = { op_id: held.op_id, kind: 'day.patch', at: '2026-01-01T00:00:00.000Z', payload: { date: date(0) } };
		await putQueueItem(held);
		for (let i = 0; i < 63; i++) await put(`/api/days/${date(i)}`);

		const left = (await allCacheForUser(user)).map((e) => e.path);
		expect(left).toContain('/api/profile');
		expect(left).toContain(`/api/days/${date(0)}`); // queued, so kept
		expect(left).not.toContain(`/api/days/${date(1)}`);
		expect(left).not.toContain(`/api/days/${date(2)}`);
		expect(left).toContain(`/api/days/${date(3)}`);
		expect(left.filter((p) => p.startsWith('/api/days/'))).toHaveLength(61);
		await clearUserData(user);
	});
});
