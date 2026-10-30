// SPDX-License-Identifier: AGPL-3.0-or-later
// Settings → Data: download the CSV and a backup, re-import the CSV (nothing changes), then restore
// the backup. Each describe starts its own server on a fresh data dir (like today.spec.ts), so the
// shared Playwright database is untouched.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync, readFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BIN = fileURLToPath(new URL('../../server/target/debug/cupful', import.meta.url));
const USER = { username: 'mover', password: 'correct horse' };
const DATE = '2020-05-01';
const MEAL_ID = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b';
const ITEM_ID = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c';
const HEADER =
	'date,meal_id,item_index,meal,description,food,portion,kcal,protein_g,carbs_g,fat_g,steps,gym_minutes,trained,weight_kg';

const randomPort = () => 20000 + Math.floor(Math.random() * 20000);

async function startCupful(port: number): Promise<ChildProcess> {
	const proc = spawn(BIN, ['serve'], {
		stdio: 'ignore',
		env: {
			...process.env,
			CUPFUL_BIND: `127.0.0.1:${port}`,
			CUPFUL_DATA_DIR: mkdtempSync(join(tmpdir(), 'cupful-e2e-data-')),
			CUPFUL_SECURE_COOKIES: 'false',
			CUPFUL_PRODUCT_LOOKUP: 'off',
			CUPFUL_PROVIDER_BASE_URL: '',
			CUPFUL_SECRET_KEY: ''
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

async function login(page: Page) {
	const res = await page.request.post('/api/auth/login', { headers: { 'X-Cupful': '1' }, data: USER });
	expect(res.ok()).toBe(true);
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Log a meal' })).toBeVisible();
}

/** A meal on a fixed old date plus a day's activity, written through the sync contract. */
async function seed(page: Page) {
	const now = new Date().toISOString();
	const res = await page.request.post('/api/sync', {
		headers: { 'X-Cupful': '1' },
		data: {
			ops: [
				{
					op_id: randomUUID(),
					kind: 'meal.upsert',
					at: now,
					payload: {
						id: MEAL_ID,
						date: DATE,
						meal_type: 'lunch',
						description: 'Rice and chicken',
						scale: 1,
						assumptions: null,
						confidence: null,
						source: 'manual',
						provider_id: null,
						items: [
							{
								id: ITEM_ID,
								name: 'Rice',
								portion_text: '1 cup',
								grams: 150,
								kcal: 195,
								protein_g: 4,
								carbs_g: 43,
								fat_g: 0.4,
								food_id: null,
								origin: 'manual'
							}
						]
					}
				},
				{
					op_id: randomUUID(),
					kind: 'day.patch',
					at: now,
					payload: { date: DATE, steps: 5000, gym_minutes: 30, trained: ['Legs'], weight_kg: 80 }
				}
			]
		}
	});
	expect(res.ok()).toBe(true);
	const { results } = (await res.json()) as { results: { status: string }[] };
	expect(results.map((r) => r.status)).toEqual(['applied', 'applied']);
}

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

test.describe('Settings → Data', () => {
	test.describe.configure({ mode: 'serial' });
	const port = randomPort();
	let server: ChildProcess;
	test.use({ baseURL: `http://127.0.0.1:${port}` });
	test.beforeAll(async () => (server = await startCupful(port)));
	test.afterAll(() => void server?.kill());
	test.beforeEach(({ page }) => login(page));

	test('export, re-import the same CSV, then restore a backup', async ({ page }) => {
		await seed(page);
		expect((await (await page.request.get(`/api/meals?date=${DATE}`)).json()).meals).toHaveLength(1);

		await page.goto('/settings');
		const data = page.getByRole('region', { name: 'Data' });
		await expect(data).toBeVisible();

		// CSV: header plus one row per meal item, with the day's activity on it.
		const csvEvent = page.waitForEvent('download');
		await data.getByRole('button', { name: 'Download CSV' }).click();
		const csvPath = await (await csvEvent).path();
		expect(csvPath).not.toBeNull();
		const csv = readFileSync(csvPath!, 'utf8');
		expect(csv.trimEnd().split(/\r?\n/)[0]).toBe(HEADER);
		const row = csv.split(/\r?\n/).find((line) => line.includes(MEAL_ID));
		expect(row).toBeDefined();
		expect(row!.startsWith(`${DATE},${MEAL_ID},0,`)).toBe(true);
		expect(row).toContain('Rice and chicken');
		expect(row).toContain('5000');
		expect(row).toContain('80');

		// Re-importing the same file changes nothing.
		await data.getByLabel('Import CSV').setInputFiles({
			name: 'fuel-ledger.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(csv)
		});
		await expect(data.getByRole('status')).toContainText('unchanged');
		expect((await (await page.request.get(`/api/meals?date=${DATE}`)).json()).meals).toHaveLength(1);

		// A backup is a JSON document, taken from the same section.
		const backupEvent = page.waitForEvent('download');
		await data.getByRole('button', { name: 'Download backup' }).click();
		const backupPath = await (await backupEvent).path();
		expect(backupPath).not.toBeNull();
		const backup = JSON.parse(readFileSync(backupPath!, 'utf8'));
		expect(typeof backup).toBe('object');

		// Restore asks first, then reports what it replaced.
		let asked = '';
		page.once('dialog', (dialog) => {
			asked = dialog.message();
			void dialog.accept();
		});
		await data.getByLabel('Restore backup').setInputFiles({
			name: 'cupful-backup.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify(backup))
		});
		await expect.poll(() => asked).toContain('Replace all logged data');
		await expect(data.getByRole('status')).toContainText(/Restored/);
		await expect(data.getByRole('status')).toContainText('1 meal');

		// The restored log has the same single meal, not a duplicate.
		const after = (await (await page.request.get(`/api/meals?date=${DATE}`)).json()).meals;
		expect(after).toHaveLength(1);
		expect(after[0].description).toBe('Rice and chicken');
		const restoredDay = await (await page.request.get(`/api/days/${DATE}`)).json();
		expect(restoredDay.steps).toBe(5000);
		await noSideScroll(page);
	});
});
