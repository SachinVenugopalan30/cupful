// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared harness for the M6 offline/PWA specs: a private `cupful` server with its own data dir
// (so the shared Playwright webServer and its data dir are never touched), a second demo account
// for account-switch tests, and small IndexedDB/service-worker helpers.
import { expect, type Page } from '@playwright/test';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const BIN = fileURLToPath(new URL('../../../server/target/debug/cupful', import.meta.url));

export const USER_A = { username: 'eater', password: 'correct horse' };
/** Created with `seed-demo`; the password is random and printed once by the CLI. */
export const USER_B = { username: 'demo', password: '' };

export type CupfulServer = {
	proc: ChildProcess;
	base: string;
	port: number;
	dataDir: string;
	stop: () => void;
};

const timezone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;

export const randomPort = () => 20000 + Math.floor(Math.random() * 20000);

/** The user's local date, `YYYY-MM-DD` (matches the profile timezone the tests set up). */
export const localDate = () => new Date().toLocaleDateString('en-CA');

function serverEnv(port: number, dataDir: string, extra: Record<string, string>) {
	return {
		...process.env,
		CUPFUL_BIND: `127.0.0.1:${port}`,
		CUPFUL_DATA_DIR: dataDir,
		CUPFUL_SECURE_COOKIES: 'false',
		CUPFUL_PRODUCT_LOOKUP: 'off',
		CUPFUL_PROVIDER_BASE_URL: '',
		CUPFUL_SECRET_KEY: '',
		...extra,
	};
}

/** Starts `cupful serve` on a fresh data dir and runs the first-run setup for USER_A. */
export async function startCupful(
	port = randomPort(),
	env: Record<string, string> = {}
): Promise<CupfulServer> {
	const dataDir = mkdtempSync(join(tmpdir(), 'cupful-e2e-offline-'));
	const proc = spawn(BIN, ['serve'], {
		stdio: 'ignore',
		env: serverEnv(port, dataDir, env),
	});
	const base = `http://127.0.0.1:${port}`;

	let up = false;
	for (let i = 0; i < 200; i++) {
		if (
			await fetch(`${base}/api/health`).then(
				(r) => r.ok,
				() => false
			)
		) {
			up = true;
			break;
		}
		await new Promise((r) => setTimeout(r, 100));
	}
	expect(up, 'cupful server did not become healthy').toBe(true);

	const res = await fetch(`${base}/api/auth/setup`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', 'X-Cupful': '1' },
		body: JSON.stringify({ ...USER_A, timezone: timezone() }),
	});
	expect(res.ok, 'first-run setup failed').toBe(true);

	return { proc, base, port, dataDir, stop: () => void proc.kill() };
}

/**
 * Adds the `demo` user (and one day of their history) with `seed-demo`. It runs against the live
 * data dir; SQLite is in WAL mode with a busy timeout, so a short CLI write is safe.
 */
export function seedDemoUser(server: CupfulServer): {
	username: string;
	password: string;
} {
	const res = spawnSync(BIN, ['seed-demo', '--days', '1', '--seed', '9'], {
		encoding: 'utf8',
		env: serverEnv(server.port, server.dataDir, {}),
		timeout: 60_000,
	});
	expect(res.status, `seed-demo failed: ${res.stderr}`).toBe(0);
	const password = /Password \(shown once\): (\S+)/.exec(res.stdout)?.[1];
	expect(password, `seed-demo printed no password: ${res.stdout}${res.stderr}`).toBeTruthy();
	USER_B.password = password!;
	return { ...USER_B };
}

/** Logs in through the API (the browser context keeps the cookie), then opens Today. */
export async function login(page: Page, user = USER_A): Promise<void> {
	const res = await page.request.post('/api/auth/login', {
		headers: { 'X-Cupful': '1' },
		data: user,
	});
	expect(res.ok(), `login failed for ${user.username}`).toBe(true);
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
}

/** Saves a profile so the budget/day screens work and the weight unit is kg. */
export async function saveProfile(page: Page): Promise<void> {
	const res = await page.request.put('/api/profile', {
		headers: { 'X-Cupful': '1' },
		data: {
			timezone: timezone(),
			weight_unit: 'kg',
			height_unit: 'cm',
			sex: 'male',
			birth_year: 1990,
			height_cm: 180,
			start_weight_kg: 80,
			goal_kg: 75,
			pace_kg_per_week: 0.5,
			typical_steps: 8000,
		},
	});
	expect(res.ok()).toBe(true);
}

/** One complete meal through the real sync contract, online. Returns its id. */
export async function seedMeal(page: Page, date: string, description: string): Promise<string> {
	const mealId = randomUUID();
	const res = await page.request.post('/api/sync', {
		headers: { 'X-Cupful': '1' },
		data: {
			ops: [
				{
					op_id: randomUUID(),
					kind: 'meal.upsert',
					at: new Date().toISOString(),
					payload: {
						id: mealId,
						date,
						meal_type: 'lunch',
						description,
						scale: 1,
						assumptions: null,
						confidence: 'medium',
						source: 'manual',
						provider_id: null,
						items: [
							{
								id: randomUUID(),
								name: 'test rice',
								portion_text: '1 cup',
								grams: 150,
								kcal: 200,
								protein_g: 4,
								carbs_g: 40,
								fat_g: 1,
								food_id: null,
								origin: 'manual',
							},
						],
					},
				},
			],
		},
	});
	expect(res.ok()).toBe(true);
	const body = (await res.json()) as { results: { status: string }[] };
	expect(body.results[0]?.status).toBe('applied');
	return mealId;
}

/** The signed-in user's id (the key for the queue and read cache). */
export async function currentUserId(page: Page): Promise<number> {
	const state = (await (await page.request.get('/api/auth/state')).json()) as {
		user: { id: number } | null;
	};
	expect(state.user).not.toBeNull();
	return state.user!.id;
}

/**
 * Waits until a service worker controls this page, reloading once if the first load registered
 * the worker without a controller. Call while online.
 */
export async function waitForController(page: Page): Promise<void> {
	await page.evaluate(async () => {
		await navigator.serviceWorker.ready;
	});
	if (!(await page.evaluate(() => !!navigator.serviceWorker.controller))) {
		await page.reload();
	}
	await page.waitForFunction(() => !!navigator.serviceWorker.controller, undefined, {
		timeout: 30_000,
	});
}

// ---- IndexedDB helpers (`cupful-offline`: queue, cache, meta) ----

type QueueRecord = {
	op_id: string;
	user_id: number;
	op: { kind: string; [key: string]: unknown };
	status: string;
	error?: string;
	queued_at: number;
};

/** Every queue record, oldest first. */
export function queueRecords(page: Page): Promise<QueueRecord[]> {
	return page.evaluate(async () => {
		const db = await new Promise<IDBDatabase>((resolve, reject) => {
			const req = indexedDB.open('cupful-offline');
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
		});
		try {
			const all = await new Promise<QueueRecord[]>((resolve, reject) => {
				const req = db.transaction('queue').objectStore('queue').getAll();
				req.onsuccess = () => resolve(req.result as QueueRecord[]);
				req.onerror = () => reject(req.error);
			});
			return all.sort((a, b) => a.queued_at - b.queued_at);
		} finally {
			db.close();
		}
	});
}

/** How many records are in the IndexedDB queue. */
export function queueCount(page: Page): Promise<number> {
	return queueRecords(page).then((ops) => ops.length);
}

/** Rewrites the newest queue record's op.kind, keeping everything else. */
export function breakNewestQueueOp(page: Page, kind = 'broken'): Promise<void> {
	return page.evaluate(async (newKind) => {
		const db = await new Promise<IDBDatabase>((resolve, reject) => {
			const req = indexedDB.open('cupful-offline');
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
		});
		try {
			const tx = db.transaction('queue', 'readwrite');
			const store = tx.objectStore('queue');
			const all = (
				await new Promise<QueueRecord[]>((resolve, reject) => {
					const req = store.getAll();
					req.onsuccess = () => resolve(req.result as QueueRecord[]);
					req.onerror = () => reject(req.error);
				})
			).sort((a, b) => a.queued_at - b.queued_at);
			if (!all.length) throw new Error('the queue is empty');
			const newest = all.at(-1)!;
			newest.op.kind = newKind;
			await new Promise<void>((resolve, reject) => {
				const req = store.put(newest);
				req.onsuccess = () => resolve();
				req.onerror = () => reject(req.error);
			});
			await new Promise<void>((resolve, reject) => {
				tx.oncomplete = () => resolve();
				tx.onerror = () => reject(tx.error);
			});
		} finally {
			db.close();
		}
	}, kind);
}

/** How many cached API bodies are stored for one user. */
export function cacheCountForUser(page: Page, userId: number): Promise<number> {
	return page.evaluate(async (id) => {
		const db = await new Promise<IDBDatabase>((resolve, reject) => {
			const req = indexedDB.open('cupful-offline');
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
		});
		try {
			const count = await new Promise<number>((resolve, reject) => {
				const req = db.transaction('cache').objectStore('cache').index('by_user').count(id);
				req.onsuccess = () => resolve(req.result);
				req.onerror = () => reject(req.error);
			});
			return count;
		} finally {
			db.close();
		}
	}, userId);
}

/**
 * Per-page offline emulation (Chromium). Unlike `context.setOffline`, this leaves the other tab
 * online so it can log out and in while the first tab still holds the queue.
 */
export async function setPageOffline(page: Page, offline: boolean): Promise<void> {
	const session = await page.context().newCDPSession(page);
	await session.send('Network.enable');
	await session.send('Network.emulateNetworkConditions', {
		offline,
		latency: 0,
		downloadThroughput: offline ? 0 : -1,
		uploadThroughput: offline ? 0 : -1,
	});
	await page.evaluate((isOffline) => {
		window.dispatchEvent(new Event(isOffline ? 'offline' : 'online'));
	}, offline);
}
