// SPDX-License-Identifier: AGPL-3.0-or-later
// Documentation screenshots (PLAN.md §17, D9). Skipped unless SCREENSHOTS is set. It runs its own
// server and data dir, seeds a profile with two weeks of history, and writes three PNGs under
// docs/screenshots/. Never wired into CI: run it by hand with `SCREENSHOTS=1 npx playwright test`.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	login,
	randomPort,
	seedDemoUser,
	startCupful,
	type CupfulServer
} from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });
test.skip(!process.env.SCREENSHOTS, 'set SCREENSHOTS=1 to capture docs screenshots');
test.use({ reducedMotion: 'reduce' });

const OUT = fileURLToPath(new URL('../../docs/screenshots', import.meta.url));
const port = randomPort();
test.use({ baseURL: `http://127.0.0.1:${port}` });

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
	// The demo user has 90 days of meals, budgets, steps and weigh-ins.
	const demo = seedDemoUser(server, 90);
	await login(page, demo);

	await page.setViewportSize({ width: 360, height: 800 });
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByTestId('meal').first()).toBeVisible();

	await setTheme(page, 'light');
	await expect(page.getByTestId('meal').first()).toBeVisible();
	await page.screenshot({ path: join(OUT, 'today-360-light.png') });

	await setTheme(page, 'dark');
	await expect(page.getByTestId('meal').first()).toBeVisible();
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
