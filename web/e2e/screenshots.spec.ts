// SPDX-License-Identifier: AGPL-3.0-or-later
// Documentation screenshots (PLAN.md §17, D9). Skipped unless SCREENSHOTS is set. It runs its own
// server and data dir, seeds a profile with two weeks of history, and writes three PNGs under
// docs/screenshots/. Never wired into CI: run it by hand with `SCREENSHOTS=1 npx playwright test`.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	localDate,
	login,
	randomPort,
	saveProfile,
	seedDay,
	seedMeal,
	startCupful,
	type CupfulServer
} from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });
test.skip(!process.env.SCREENSHOTS, 'set SCREENSHOTS=1 to capture docs screenshots');

const OUT = fileURLToPath(new URL('../../docs/screenshots', import.meta.url));
const port = randomPort();
test.use({ baseURL: `http://127.0.0.1:${port}` });

const daysAgo = (n: number) =>
	new Date(Date.now() - n * 86_400_000).toLocaleDateString('en-CA');

let server: CupfulServer;

test.beforeAll(async () => {
	server = await startCupful(port);
	mkdirSync(OUT, { recursive: true });
});

test.afterAll(() => server?.stop());

/** Applies a theme through localStorage (the app reads it before first paint), then reloads. */
async function setTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	await page.evaluate((value) => localStorage.setItem('cupful-theme', value), theme);
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
}

test('capture Today in light and dark at 360, then Trends at 1280', async ({ page }) => {
	await login(page);
	await saveProfile(page);

	// Two weeks of steps and a slow downward weight curve, so the Trends charts have shape.
	for (let n = 13; n >= 0; n--) {
		await seedDay(page, daysAgo(n), {
			steps: 6200 + ((n * 1100) % 5200),
			weight_kg: Math.round((80.4 - (13 - n) * 0.11) * 10) / 10
		});
	}
	// Older intake so the Trends intake chart is not a single bar.
	for (let n = 7; n >= 1; n--) {
		const date = daysAgo(n);
		await seedMeal(page, date, 'Greek yogurt, berries and granola', 'breakfast');
		await seedMeal(page, date, 'Chicken, rice and broccoli bowl', 'lunch');
	}
	const today = localDate();
	await seedMeal(page, today, 'Greek yogurt, berries and granola', 'breakfast');
	await seedMeal(page, today, 'Chicken, rice and broccoli bowl', 'lunch');
	await seedMeal(page, today, 'Salmon with roast potatoes and greens', 'dinner');

	await page.setViewportSize({ width: 360, height: 800 });
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByTestId('meal').filter({ hasText: 'Salmon' })).toBeVisible();

	await setTheme(page, 'light');
	await expect(page.getByTestId('meal').filter({ hasText: 'Salmon' })).toBeVisible();
	await page.screenshot({ path: join(OUT, 'today-360-light.png') });

	await setTheme(page, 'dark');
	await expect(page.getByTestId('meal').filter({ hasText: 'Salmon' })).toBeVisible();
	await page.screenshot({ path: join(OUT, 'today-360-dark.png') });

	// Trends is a wide screen in the docs.
	await page.setViewportSize({ width: 1280, height: 1400 });
	await page.goto('/trends');
	await expect(page.getByRole('heading', { name: 'Trends', level: 1 })).toBeVisible();
	await setTheme(page, 'light');
	await expect(page.getByRole('region', { name: 'Weight' }).locator('.plot svg').first()).toBeVisible();
	await expect(page.getByText(/kcal eaten/)).toBeVisible();
	await page.screenshot({ path: join(OUT, 'trends-1280.png') });
});
