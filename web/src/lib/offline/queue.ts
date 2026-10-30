// SPDX-License-Identifier: AGPL-3.0-or-later
// The offline write queue: every logged-data op is stored in IndexedDB and sent to /api/sync by
// `flush`, which is the only place in the app that posts to that endpoint besides the
// activeUserId-null fallback in `sendOps` (used before an account is loaded).
//
// The queue is per user and refuses to flush when the session belongs to someone else. Flush is
// serialised with the Web Locks API (or a module promise where locks are missing), so two tabs or
// two callers can't race the same ops.

import type { Day, Meal } from '../api.ts';
import type { Op, OpResult } from '../sync/index.ts';
import {
	allQueueForUser,
	clearUserData,
	deleteQueueItem,
	getDb,
	putQueueItem,
	putQueueItems
} from './db.ts';
import { deleteCachedBody, putCachedBody } from './cache.ts';
import { overlayDay, overlayMeals } from './overlay.ts';
import { bumpDataVersion, connection, queueState } from './state.svelte.ts';
import type { QueuedOp } from './types.ts';

const CHUNK = 100;
const BACKOFF_START = 1_000;
const BACKOFF_MAX = 60_000;

let onlineHandler: (() => void) | null = null;
// ponytail: backoff lives in memory; a reload starts at 1s again, which is fine for a retry timer.
let backoffTimer: ReturnType<typeof setTimeout> | null = null;
let backoffMs = 0;
let serial: Promise<unknown> = Promise.resolve();

function isOnline(): boolean {
	return typeof navigator === 'undefined' || navigator.onLine !== false;
}

function cancelBackoff(): void {
	if (backoffTimer !== null) clearTimeout(backoffTimer);
	backoffTimer = null;
	backoffMs = 0;
}

function scheduleBackoff(): void {
	if (backoffTimer !== null) return;
	backoffMs = backoffMs === 0 ? BACKOFF_START : Math.min(backoffMs * 2, BACKOFF_MAX);
	backoffTimer = setTimeout(() => {
		backoffTimer = null;
		void flush(connection.activeUserId ?? undefined);
	}, backoffMs);
}

/** Reads the queue back into reactive state and recounts pending/failed. */
async function refresh(userId = connection.activeUserId): Promise<void> {
	if (userId === null) {
		queueState.ops = [];
		connection.pending = 0;
		connection.failed = 0;
		return;
	}
	const ops = await allQueueForUser(userId);
	queueState.ops = ops;
	connection.pending = ops.filter((o) => o.status === 'pending' || o.status === 'sending').length;
	connection.failed = ops.filter(
		(o) => o.status === 'stale' || o.status === 'rejected' || o.status === 'failed'
	).length;
}

/** Loads a user's queue and starts flushing. Call after login and on boot with a cached user. */
export async function start(userId: number): Promise<void> {
	connection.activeUserId = userId;
	connection.paused = null;
	cancelBackoff();
	await refresh(userId);

	if (!onlineHandler && typeof window !== 'undefined') {
		onlineHandler = () => {
			connection.online = true;
			cancelBackoff();
			if (connection.activeUserId !== null) void flush(connection.activeUserId);
		};
		window.addEventListener('online', onlineHandler);
	}

	if (isOnline()) void flush(userId).catch(() => {});
	else connection.paused = 'offline';
}

/** Stops timers and detaches this session's queue. */
export function stop(): void {
	cancelBackoff();
	if (onlineHandler && typeof window !== 'undefined') {
		window.removeEventListener('online', onlineHandler);
	}
	onlineHandler = null;
	connection.activeUserId = null;
	connection.paused = null;
	queueState.ops = [];
	connection.pending = 0;
	connection.failed = 0;
}

// ---- direct post (storage fallback) ----

async function postDirect(ops: Op[]): Promise<OpResult[]> {
	const outcome = await postChunk(ops);
	if (outcome.kind !== 'ok') throw new Error('Could not save the change.');
	return outcome.results;
}

/**
 * Stores ops for later sending. If IndexedDB refuses the write (a full quota, say) and we're
 * online, the ops are posted straight away and their results are returned; offline there is
 * nowhere to put them, so this throws. `null` means the ops are in the queue.
 */
export async function enqueue(ops: Op[], userId: number): Promise<OpResult[] | null> {
	if (!ops.length) return null;
	const now = Date.now();
	const items: QueuedOp[] = ops.map((op) => ({
		op_id: op.op_id,
		user_id: userId,
		op,
		status: 'pending',
		queued_at: now,
		attempts: 0
	}));

	try {
		await putQueueItems(items);
	} catch {
		if (!isOnline()) {
			throw new Error(
				'This device has no room to store the change offline. Free some space and try again.'
			);
		}
		const results = await postDirect(ops);
		await refresh();
		return results;
	}

	await refresh();
	return null;
}

// ---- flush ----

/** Runs `fn` under the cross-tab lock when available, else under a module promise. */
async function withLock<T>(fn: () => Promise<T>): Promise<T> {
	const locks = typeof navigator !== 'undefined' ? navigator.locks : undefined;
	if (locks && typeof locks.request === 'function') {
		return (await locks.request('cupful-sync', fn)) as T;
	}
	const previous = serial;
	let release!: () => void;
	serial = new Promise<void>((resolve) => (release = resolve));
	await previous;
	try {
		return await fn();
	} finally {
		release();
	}
}

/**
 * `auth` means signed out (the session expired), `identity` means a different account is signed
 * in. Keeping them apart is what makes the bar say "sign in" instead of "another account".
 */
async function identityMatches(
	userId: number
): Promise<'ok' | 'auth' | 'identity' | 'network'> {
	try {
		const res = await fetch('/api/auth/state', {
			cache: 'no-store',
			credentials: 'same-origin',
		});
		if (!res.ok) return 'network';
		const state = (await res.json()) as { user?: { id?: number } | null };
		if (!state?.user) return 'auth';
		return state.user.id === userId ? 'ok' : 'identity';
	} catch {
		return 'network';
	}
}

/** Applies one landed op to the cached read bodies before its queue row goes away. */
async function writeThrough(userId: number, item: QueuedOp): Promise<void> {
	if (item.op.kind === 'day.patch') {
		const date = item.op.payload.date;
		await updateDays(userId, item, date);
		await deleteCachedBody(userId, `/api/budget/${date}`);
		const db = await getDb();
		const entries = await db.getAllFromIndex('cache', 'by_user', userId);
		for (const entry of entries) {
			if (entry.path.startsWith('/api/trends')) await deleteCachedBody(userId, entry.path);
		}
		return;
	}
	await updateMeals(userId, item);
}

/** The list inside a cached body's named field. */
function listFrom<T>(body: unknown, field: string): T[] | null {
	const inner = (body as Record<string, unknown> | null)?.[field];
	return Array.isArray(inner) ? (inner as T[]) : null;
}

async function updateMeals(userId: number, item: QueuedOp): Promise<void> {
	const db = await getDb();
	const entries = await db.getAllFromIndex('cache', 'by_user', userId);
	// A delete doesn't care about the date; the overlay removes by id for every cached day.
	const upsertDate = item.op.kind === 'meal.upsert' ? item.op.payload.date : '';
	// The row may be `failed` when it is retried; the overlay only applies live rows.
	const pending = { ...item, status: 'pending' as const };

	for (const entry of entries) {
		if (!entry.path.startsWith('/api/meals?')) continue;
		const list = listFrom<Meal>(entry.body, 'meals');
		if (list === null) continue;

		const day = new URL(entry.path, 'http://cupful.invalid').searchParams.get('date') ?? upsertDate;
		if (item.op.kind === 'meal.upsert' && day !== item.op.payload.date) continue;
		const next = overlayMeals(list, [pending], day);
		await putCachedBody(userId, entry.path, { ...(entry.body as object), meals: next });
	}
}

async function updateDays(userId: number, item: QueuedOp, date: string): Promise<void> {
	const db = await getDb();
	const entries = await db.getAllFromIndex('cache', 'by_user', userId);
	const pending = { ...item, status: 'pending' as const };

	for (const entry of entries) {
		if (entry.path === `/api/days/${date}`) {
			const day = entry.body as Day | null;
			if (!day || typeof day !== 'object') continue;
			await putCachedBody(userId, entry.path, overlayDay(day, [pending], date));
			continue;
		}
		if (!entry.path.startsWith('/api/days?')) continue;
		const params = new URL(entry.path, 'http://cupful.invalid').searchParams;
		const from = params.get('from');
		const to = params.get('to');
		if (!from || !to || date < from || date > to) continue;

		const list = listFrom<Day>(entry.body, 'days');
		if (list === null) continue;
		const next = list.map((d) => (d.date === date ? overlayDay(d, [pending], date) : d));
		await putCachedBody(userId, entry.path, { ...(entry.body as object), days: next });
	}
}

type SendOutcome =
	| { kind: 'ok'; results: OpResult[] }
	| { kind: 'network' }
	| { kind: 'auth' }
	| { kind: 'error'; message: string };

async function postChunk(ops: Op[]): Promise<SendOutcome> {
	let res: Response;
	try {
		res = await fetch('/api/sync', {
			method: 'POST',
			credentials: 'same-origin',
			headers: { 'X-Cupful': '1', 'Content-Type': 'application/json' },
			body: JSON.stringify({ ops }),
		});
	} catch {
		return { kind: 'network' };
	}

	if (res.status === 401) return { kind: 'auth' };
	if (res.status === 429 || res.status >= 500) return { kind: 'network' };
	if (!res.ok)
		return {
			kind: 'error',
			message: `The server answered with an error (${res.status}).`,
		};

	try {
		const body = (await res.json()) as { results?: OpResult[] };
		return { kind: 'ok', results: body.results ?? [] };
	} catch {
		return { kind: 'network' };
	}
}

/** Applies one server result to its queue row, the cached reads and the caller's result map. */
async function applyResult(
	userId: number,
	item: QueuedOp,
	result: OpResult,
	results: Map<string, OpResult>
): Promise<void> {
	if (result.status === 'applied' || result.status === 'duplicate') {
		try {
			await writeThrough(userId, item);
		} catch {
			// The server has it; a failed cache write must not make us send it again.
		}
		await deleteQueueItem(item.op_id);
	} else if (result.status === 'stale') {
		await putQueueItem({
			...item,
			status: 'stale',
			error: 'A newer version replaced this change.',
		});
	} else if (result.status === 'rejected') {
		await putQueueItem({
			...item,
			status: 'rejected',
			error: result.error ?? 'The server rejected this change.',
		});
	}
	results.set(item.op_id, {
		op_id: item.op_id,
		status: result.status,
		...(result.error ? { error: result.error } : {}),
	});
}

async function doFlush(userId: number): Promise<Map<string, OpResult>> {
	const results = new Map<string, OpResult>();
	if (!isOnline()) {
		connection.paused = 'offline';
		await refresh(userId);
		return results;
	}
	// A pending backoff means a recent attempt failed; let the timer or an explicit retry try again.
	if (backoffTimer !== null) return results;

	const ops = (await allQueueForUser(userId)).filter(
		(o) => o.status === 'pending' || o.status === 'failed'
	);
	if (!ops.length) {
		if (connection.paused === 'offline') connection.paused = null;
		await refresh(userId);
		return results;
	}

	const identity = await identityMatches(userId);
	if (identity === 'auth' || identity === 'identity') {
		connection.paused = identity;
		await refresh(userId);
		return results;
	}
	if (identity === 'network') {
		connection.paused = 'offline';
		scheduleBackoff();
		await refresh(userId);
		return results;
	}

	for (let i = 0; i < ops.length; i += CHUNK) {
		const chunk = ops.slice(i, i + CHUNK);
		const outcome = await postChunk(chunk.map((o) => o.op));

		if (outcome.kind === 'auth') {
			connection.paused = 'auth';
			break;
		}
		if (outcome.kind === 'network') {
			connection.paused = 'offline';
			scheduleBackoff();
			break;
		}
		if (outcome.kind === 'error') {
			// The batch reached the server but was rejected as a whole. Resend its ops one at a
			// time so one bad change is marked failed instead of every change beside it.
			connection.paused = null;
			for (const item of chunk) {
				const single = await postChunk([item.op]);
				if (single.kind === 'auth') {
					connection.paused = 'auth';
					break;
				}
				if (single.kind === 'network') {
					connection.paused = 'offline';
					scheduleBackoff();
					break;
				}
				if (single.kind === 'error') {
					await putQueueItem({ ...item, status: 'failed', error: single.message });
					continue;
				}
				const result = single.results.find((r) => r.op_id === item.op_id) ?? single.results[0];
				if (result) await applyResult(userId, item, result, results);
			}
			await refresh(userId);
			break;
		}

		cancelBackoff();
		connection.paused = null;

		const byId = new Map(outcome.results.map((r) => [r.op_id, r]));
		for (const [index, item] of chunk.entries()) {
			const result = byId.get(item.op_id) ?? outcome.results[index];
			if (!result) continue;
			await applyResult(userId, item, result, results);
		}
		await refresh(userId);
	}

	await refresh(userId);
	bumpDataVersion();
	return results;
}

/**
 * Sends everything pending for `userId` (default: the active user), oldest first. Returns the
 * server's result per op_id for the ops this flush saw; network-paused ops are simply missing.
 */
export async function flush(userId?: number): Promise<Map<string, OpResult>> {
	const uid = userId ?? connection.activeUserId;
	if (uid === null) return new Map();
	return withLock(() => doFlush(uid));
}

/** Force a flush, even if a backoff is pending. */
export async function syncNow(): Promise<void> {
	cancelBackoff();
	await flush(connection.activeUserId ?? undefined);
}

export async function retry(opId: string): Promise<void> {
	const item = queueState.ops.find((o) => o.op_id === opId);
	if (!item) return;
	await putQueueItem({ ...item, status: 'pending', error: undefined });
	cancelBackoff();
	await flush(item.user_id);
}

export async function discard(opId: string): Promise<void> {
	await deleteQueueItem(opId);
	await refresh();
	bumpDataVersion();
}

/** Drops a user's queue and cached reads (logout, or "start over" before switching accounts). */
export async function clearUser(userId: number): Promise<void> {
	await clearUserData(userId);
	if (connection.activeUserId === userId) {
		queueState.ops = [];
		connection.pending = 0;
		connection.failed = 0;
	}
	bumpDataVersion();
}

/** How many ops are still in the queue (waiting or needing attention) for this user. */
export async function countFor(userId: number): Promise<number> {
	return (await allQueueForUser(userId)).length;
}
