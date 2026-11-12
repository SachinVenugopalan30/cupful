// SPDX-License-Identifier: AGPL-3.0-or-later
// An offline usual Log must not come back as a second Log. The chip hides when the same usual was
// logged today, and the history read is served from the cache, so the queued upsert has to count
// too. This spec owns its server: no other test has already logged the top usual today.
import { expect, test } from '@playwright/test';
import {
	localDate,
	login,
	randomPort,
	seedMeal,
	startCupful,
	waitForController,
	type CupfulServer,
	type MealType
} from './helpers/server';

const port = randomPort();
let server: CupfulServer;

test.use({ baseURL: `http://127.0.0.1:${port}` });

test.beforeAll(async () => {
	server = await startCupful(port);
});
test.afterAll(() => server.stop());

function slot(hour: number): MealType {
	if (hour >= 5 && hour < 11) return 'breakfast';
	if (hour >= 11 && hour < 15) return 'lunch';
	if (hour >= 18 && hour < 22) return 'dinner';
	return 'snack';
}

function daysAgo(n: number): string {
	const d = new Date();
	d.setDate(d.getDate() - n);
	return d.toLocaleDateString('en-CA');
}

test('an offline Log stays hidden after a reload', async ({ page }) => {
	await login(page);
	const mealType = slot(new Date().getHours());
	const date = localDate();
	for (let i = 1; i <= 4; i++) await seedMeal(page, daysAgo(i), 'Offline oats', mealType);

	// Load once online so the shell, the day read and the history read are cached.
	await page.goto('/');
	const usual = page.getByTestId('usual');
	await expect(usual).toContainText('Offline oats');
	await waitForController(page);

	await page.context().setOffline(true);
	await expect(page.getByTestId('sync-bar')).toContainText('Offline');
	await usual.getByRole('button', { name: 'Log', exact: true }).click();
	await expect(usual).toContainText('Usual queued. Waiting to sync.');
	await expect(page.getByTestId('meal')).toHaveCount(1);

	// Reload while still offline: the history read is the cached copy, but the queue holds the log.
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByTestId('meal')).toHaveCount(1);
	await expect(page.getByTestId('usual')).toHaveCount(0);

	// The day round trip remounts the chip with the same cached history; it must stay hidden.
	await page.getByRole('button', { name: 'Previous day' }).click();
	await page.getByRole('button', { name: 'Back to today' }).click();
	await expect(page.getByTestId('meal')).toHaveCount(1);
	await expect(page.getByTestId('usual')).toHaveCount(0);

	// Reconnecting lands the queued meal, and it stays the only one.
	await page.context().setOffline(false);
	await expect(page.getByTestId('sync-bar')).toBeHidden({ timeout: 30_000 });
	const body = (await (await page.request.get(`/api/meals?date=${date}`)).json()) as {
		meals: { description: string }[];
	};
	expect(body.meals.filter((m) => m.description === 'Offline oats')).toHaveLength(1);
});
