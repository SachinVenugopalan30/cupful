// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared fixtures, fetch stubs and lifecycle hooks for the offline queue tests. Not a test file
// itself: each queue test file calls `installQueueTestHooks()` once and imports the builders it
// needs.

import { afterEach, beforeEach, vi } from 'vitest';
import type { Day, Meal, MealItem } from '../api.ts';
import type { MealIn, Op, OpResult } from '../sync/index.ts';
import { clearUserData } from './db.ts';
import { start, stop } from './queue.ts';

let userSeq = 20_000;
let opSeq = 0;
const users: number[] = [];

/** A fresh user id, cleaned up after the test. */
export function newUser(): number {
	const id = ++userSeq;
	users.push(id);
	return id;
}

export function opId(): string {
	opSeq++;
	return `op-${String(opSeq).padStart(5, '0')}`;
}

export function json(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'Content-Type': 'application/json' },
	});
}

export function authState(id: number | null): Response {
	return json({
		needs_setup: false,
		user: id === null ? null : { id, username: 'u', is_admin: false },
	});
}

export function deleteOp(): Op {
	return {
		op_id: opId(),
		kind: 'meal.delete',
		at: '2026-10-29T12:00:00.000Z',
		payload: { id: 'm1' },
	};
}

export function item(id: string, kcal: number): MealItem {
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

export function mealIn(id: string, date: string, items: MealItem[]): MealIn {
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

export function meal(id: string, date: string, items: MealItem[]): Meal {
	return {
		...mealIn(id, date, items),
		updated_at: 1,
		totals: { kcal: 0, protein_g: 0, carbs_g: 0, fat_g: 0 },
	};
}

export function day(date: string, steps: number | null): Day {
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

export type Post = { ops: Op[] };
type Handler = (url: string, init?: RequestInit) => Response | Promise<Response>;

export function stubFetch(handler: Handler): ReturnType<typeof vi.fn> {
	const fn = vi.fn(handler);
	vi.stubGlobal('fetch', fn);
	return fn;
}

/** A fetch that passes the identity guard for `userId` and answers every chunk with `answer`. */
export function serving(userId: number, answer: (ops: Op[]) => OpResult[], posts?: Post[]) {
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

export const applied = (ops: Op[]): OpResult[] =>
	ops.map((op) => ({ op_id: op.op_id, status: 'applied' }));

export const calls = (fn: ReturnType<typeof vi.fn>, url: string) =>
	fn.mock.calls.filter(([input]) => input === url).length;

/**
 * Starts the queue while offline, so `start` doesn't flush in the background, then goes online.
 * Tests drive `flush` themselves.
 */
export async function boot(userId: number): Promise<void> {
	await start(userId);
	(navigator as { onLine: boolean }).onLine = true;
}

/** Fresh navigator state before each test; tears down the queue and test users afterwards. */
export function installQueueTestHooks(): void {
	beforeEach(() => {
		vi.stubGlobal('navigator', { onLine: false });
	});

	afterEach(async () => {
		stop();
		for (const id of users.splice(0)) await clearUserData(id);
		vi.unstubAllGlobals();
		vi.restoreAllMocks();
	});
}
