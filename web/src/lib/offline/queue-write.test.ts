// SPDX-License-Identifier: AGPL-3.0-or-later
// Queue writes: `enqueue` stores pending ops, landed ops are written through to the read cache,
// and `sendOps` stays the one logged-data write path.

import { describe, expect, it, vi } from 'vitest';
import type { Day, Meal } from '../api.ts';
import { dayPatch, mealDelete, mealUpsert, sendOps, type Op } from '../sync/index.ts';
import { getCachedBody, putCachedBody } from './cache.ts';
import { countFor, enqueue, flush, start, stop } from './queue.ts';
import {
	applied,
	boot,
	calls,
	day,
	deleteOp,
	installQueueTestHooks,
	item,
	json,
	meal,
	mealIn,
	newUser,
	serving,
	stubFetch,
	type Post,
} from './queue-test-helpers.ts';
import { connection, queueState } from './state.svelte.ts';

installQueueTestHooks();

describe('enqueue', () => {
	it('stores pending ops with the same timestamp, in creation order, and updates state', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = [deleteOp(), deleteOp(), deleteOp()];
		vi.spyOn(Date, 'now').mockReturnValue(1_234_567_890);
		await enqueue(ops, userId);

		expect(queueState.ops.map((o) => o.op_id)).toEqual(ops.map((o) => o.op_id));
		expect(queueState.ops.map((o) => o.queued_at)).toEqual([
			1_234_567_890, 1_234_567_890, 1_234_567_890,
		]);
		expect(queueState.ops.map((o) => [o.status, o.attempts])).toEqual([
			['pending', 0],
			['pending', 0],
			['pending', 0],
		]);
		expect(connection.pending).toBe(3);
		expect(await countFor(userId)).toBe(3);
	});

	it('flushes in chunks of 100, guarding identity once', async () => {
		const userId = newUser();
		await boot(userId);
		const ops = Array.from({ length: 250 }, () => deleteOp());
		await enqueue(ops, userId);
		const posts: Post[] = [];
		const fetch = serving(userId, applied, posts);

		await flush(userId);

		expect(posts.map((p) => p.ops.length)).toEqual([100, 100, 50]);
		expect(posts.flatMap((p) => p.ops.map((o) => o.op_id))).toEqual(ops.map((o) => o.op_id));
		expect(calls(fetch, '/api/auth/state')).toBe(1);
		expect(await countFor(userId)).toBe(0);
	});
});

describe('write-through', () => {
	it('updates cached meal bodies before dropping applied ops', async () => {
		const userId = newUser();
		const date = '2026-10-29';
		const path = `/api/meals?date=${date}`;
		await boot(userId);
		await putCachedBody(userId, path, {
			meals: [meal('m1', date, [item('old', 100)])],
		});
		await putCachedBody(userId, `/api/meals?date=2026-10-30`, {
			meals: [meal('other', '2026-10-30', [])],
		});

		const upsert = mealUpsert(mealIn('m1', date, [item('i1', 300), item('i2', 200)]));
		await enqueue([upsert], userId);
		serving(userId, applied);
		await flush(userId);

		const cached = await getCachedBody<{ meals: Meal[] }>(userId, path);
		expect(cached?.meals).toHaveLength(1);
		expect(cached?.meals[0].description).toBe('Queued lunch');
		expect(cached?.meals[0].totals.kcal).toBe(500);
		expect(
			(await getCachedBody<{ meals: Meal[] }>(userId, '/api/meals?date=2026-10-30'))?.meals[0].id
		).toBe('other');

		const remove = mealDelete('m1');
		await enqueue([remove], userId);
		serving(userId, applied);
		await flush(userId);

		expect((await getCachedBody<{ meals: Meal[] }>(userId, path))?.meals).toEqual([]);
	});

	it('updates cached day detail and ranges, and drops budget and trends', async () => {
		const userId = newUser();
		const date = '2026-10-29';
		await boot(userId);
		await putCachedBody(userId, `/api/days/${date}`, day(date, 1000));
		await putCachedBody(userId, `/api/days?from=2026-10-01&to=2026-10-30`, {
			days: [day(date, 1000)],
		});
		await putCachedBody(userId, '/api/days?from=2026-11-01&to=2026-11-30', {
			days: [day('2026-11-02', 50)],
		});
		await putCachedBody(userId, `/api/budget/${date}`, { date, kcal: 2000 });
		await putCachedBody(userId, '/api/trends?from=2026-10-01&to=2026-10-30', {
			days: [],
		});

		await enqueue([dayPatch(date, { steps: 4321 })], userId);
		serving(userId, applied);
		await flush(userId);

		const detail = await getCachedBody<Day>(userId, `/api/days/${date}`);
		expect(detail?.steps).toBe(4321);
		expect(detail?.steps_source).toBe('manual');
		const range = await getCachedBody<{ days: Day[] }>(
			userId,
			`/api/days?from=2026-10-01&to=2026-10-30`
		);
		expect(range?.days[0].steps).toBe(4321);
		const untouched = await getCachedBody<{ days: Day[] }>(
			userId,
			'/api/days?from=2026-11-01&to=2026-11-30'
		);
		expect(untouched?.days[0].steps).toBe(50);
		expect(await getCachedBody(userId, `/api/budget/${date}`)).toBeUndefined();
		expect(
			await getCachedBody(userId, '/api/trends?from=2026-10-01&to=2026-10-30')
		).toBeUndefined();
	});
});

describe('sendOps', () => {
	it('returns queued while offline and stores the op', async () => {
		const userId = newUser();
		await start(userId);
		const fetch = serving(userId, applied);
		const op = deleteOp();

		const results = await sendOps([op]);

		expect(results).toEqual([{ op_id: op.op_id, status: 'queued' }]);
		expect(fetch).not.toHaveBeenCalled();
		expect(await countFor(userId)).toBe(1);
	});

	it('returns the server results after a successful flush', async () => {
		const userId = newUser();
		await boot(userId);
		serving(userId, applied);
		const op = deleteOp();

		expect(await sendOps([op])).toEqual([{ op_id: op.op_id, status: 'applied' }]);
		expect(await countFor(userId)).toBe(0);
	});

	it('keeps the old direct post when no user is active', async () => {
		stop();
		const fetch = stubFetch(async (url) => {
			if (url === '/api/sync') return json({ results: [{ op_id: 'direct', status: 'applied' }] });
			throw new Error(`unexpected fetch ${url}`);
		});
		const op: Op = {
			op_id: 'direct',
			kind: 'meal.delete',
			at: '2026-10-29T12:00:00.000Z',
			payload: { id: 'm1' },
		};

		expect(await sendOps([op])).toEqual([{ op_id: 'direct', status: 'applied' }]);
		expect(calls(fetch, '/api/sync')).toBe(1);
		expect(calls(fetch, '/api/auth/state')).toBe(0);
	});
});
