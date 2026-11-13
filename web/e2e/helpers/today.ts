// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared harness for the Today specs: a private server per describe, a reused login session and
// small page helpers.
import { expect, type BrowserContext, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BIN = fileURLToPath(new URL('../../../server/target/debug/cupful', import.meta.url));
export const USER = { username: 'eater', password: 'correct horse' };
export const randomPort = () => 20000 + Math.floor(Math.random() * 20000);

export async function startCupful(port: number, env: Record<string, string>): Promise<ChildProcess> {
	const proc = spawn(BIN, ['serve'], {
		stdio: 'ignore',
		env: {
			...process.env,
			CUPFUL_BIND: `127.0.0.1:${port}`,
			CUPFUL_DATA_DIR: mkdtempSync(join(tmpdir(), 'cupful-e2e-today-')),
			CUPFUL_SECURE_COOKIES: 'false',
			CUPFUL_PRODUCT_LOOKUP: 'off',
			CUPFUL_PROVIDER_BASE_URL: '',
			CUPFUL_SECRET_KEY: '',
			...env
		}
	});
	const base = `http://127.0.0.1:${port}`;
	for (let i = 0; i < 100; i++) {
		if (await fetch(`${base}/api/health`).then((r) => r.ok, () => false)) break;
		await new Promise((r) => setTimeout(r, 100));
	}
	const res = await fetch(`${base}/api/auth/setup`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', 'X-Cupful': '1' },
		body: JSON.stringify({ ...USER, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone })
	});
	expect(res.ok).toBe(true);
	return proc;
}

// The server allows five logins per username per minute. This file now has more tests than that,
// so the first login's session is reused in the later tests' fresh contexts.
let savedAuth: Awaited<ReturnType<BrowserContext['storageState']>> | null = null;

export async function login(page: Page) {
	if (savedAuth) {
		await page.context().addCookies(savedAuth.cookies);
	} else {
		const res = await page.request.post('/api/auth/login', { headers: { 'X-Cupful': '1' }, data: USER });
		expect(res.ok()).toBe(true);
		savedAuth = await page.context().storageState();
	}
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Log a meal' })).toBeVisible();
}

export const dayTotal = (page: Page) => page.getByTestId('day-total');
export const kcalOf = async (text: string) => Number(/([\d.]+) kcal/.exec(text)![1]);

export async function fillItem(page: Page, n: number, v: [string, string, string, string, string, string]) {
	const item = page.getByRole('dialog').locator('.item').nth(n);
	const labels = ['Item name', 'Grams', 'kcal', 'Protein g', 'Carbs g', 'Fat g'];
	for (let i = 0; i < labels.length; i++) await item.getByLabel(labels[i], { exact: true }).fill(v[i]);
}

export async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

/** Forgets the saved session, for a spec whose server starts on a fresh data dir. */
export function resetSavedAuth() {
	savedAuth = null;
}
