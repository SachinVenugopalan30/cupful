// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Day, Meal, MealItem } from '../api.ts';
import {
	dayPatch,
	mealDelete,
	mealUpsert,
	sendOps,
	type MealIn,
	type Op,
	type OpResult,
} from '../sync/index.ts';
import { getCachedBody, putCachedBody } from './cache.ts';
import { clearUserData } from './db.ts';
import { clearUser, countFor, discard, enqueue, flush, retry, start, stop } from './queue.ts';
import { connection, queueState } from './state.svelte.ts';

let userSeq = 20_000;
let opSeq = 0;
const users: number[] = [];

function newUser(): number {
	const id = ++userSeq;
	users.push(id);
	return id;
}

function opId(): string {
	opSeq++;
	return `op-${String(opSeq).padStart(5, '0')}`;
}

function json(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'Content-Type': 'application/json' },
	});
}

function authState(id: number | null): Response {
	return json({
		needs_setup: false,
		user: id === null ? null : { id, username: 'u', is_admin: false },
	});
}

function deleteOp(): Op {
	return {
		op_id: opId(),
		kind: 'meal.delete',
		at: '2026-10-29T12:00:00.000Z',
		payload: { id: 'm1' },
	};
}

function item(id: string, kcal: number): MealItem {
	return {
		id,
		name: `Item ${id}`,
		portion_text: '1 serving',
		grams: 100,
		kcal,
		protein_g: 10,
		carbs_g: 20,
		fat_g: 5,
		food_id: null,
		origin: 'manual',
	};
}

function mealIn(id: string, date: string, items: MealItem[]): MealIn {
	return {
		id,
		date,
		meal_type: 'lunch',
		description: 'Queued lunch',
		scale: 1,
		assumptions: null,
		confidence: null,
		source: 'manual',
		provider_id: null,
		items,
	};
}

function meal(id: string, date: string, items: MealItem[]): Meal {
	return {
		...mealIn(id, date, items),
		updated_at: 1,
		totals: { kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 },
	};
}

function day(date: string, steps: number | null): Day {
	return {
		date,
		steps,
		steps_source: steps === null ? null : 'google',
		gym_minutes: null,
		gym_source: null,
		trained: [],
		weight_kg: null,
	};
}

type Post = { ops: Op[] };
type Handler = (url: string, init?: RequestInit) => Response | Promise<Response>;

function stubFetch(handler: Handler): ReturnType<typeof vi.fn> {
	const fn = vi.fn(handler);
	vi.stubGlobal('fetch', fn);
	return fn;
}

/** A fetch that passes the identity guard for `userId` and answers every chunk with `answer`. */
function serving(userId: number, answer: (ops: Op[]) => OpResult[], posts?: Post[]) {
	return stubFetch(async (url, init) => {
		if (url === '/api/auth/state') return authState(userId);
		if (url === '/api/sync') {
			const ops = (JSON.parse(init?.body as string) as { ops: Op[] }).ops;
			posts?.push({ ops });
			return json({ results: answer(ops) });
		}
		throw new Error(`unexpected fetch ${url}`);
	});
}

const applied = (ops: Op[]): OpResult[] =>
	ops.map((op) => ({ op_id: op.op_id, status: 'applied' }));
const calls = (fn: ReturnType<typeof vi.fn>, url: string) =>
	fn.mock.calls.filter(([input]) => input === url).length;

/**
 * Starts the queue while offline, so `start` doesn't flush in the background, then goes online.
 * Tests drive `flush` themselves.
 */
async function boot(userId: number): Promise<void> {
	await start(userId);
	(navigator as { onLine: boolean }).onLine = true;
}

beforeEach(() => {
	vi.stubGlobal('navigator', { onLine: false });
});

afterEach(async () => {
	stop();
	for (const id of users.splice(0)) await clearUserData(id);
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

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

	it('pauses identity when signed out', async () => {
		const userId = newUser();
		await boot(userId);
		await enqueue([deleteOp()], userId);
		const fetch = stubFetch(async (url) => {
			if (url === '/api/auth/state') return authState(null);
			throw new Error('must not post');
		});

		await flush(userId);

		expect(calls(fetch, '/api/sync')).toBe(0);
		expect(connection.paused).toBe('identity');
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

describe('failure handling', () => {
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
		await putCachedBody(userId, `/api/trends?from=2026-10-01&to=2026-10-30`, {
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
