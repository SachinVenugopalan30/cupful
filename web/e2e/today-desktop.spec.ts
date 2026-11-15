// SPDX-License-Identifier: AGPL-3.0-or-later
// Wide Today: the jug column holds activity tiles and the "This week" bars; phones get no week card.
// "Sync steps" lives on /activity only. Own server with a demo user's 14 days of data.
import { expect, test } from '@playwright/test';
import { login, randomPort, seedDay, seedDemoUser, startCupful, type CupfulServer } from './helpers/server';

const port = randomPort();
let server: CupfulServer;

test.use({ baseURL: `http://127.0.0.1:${port}` });

test.beforeAll(async () => {
	server = await startCupful(port);
});
test.afterAll(() => server.stop());

test('wide Today shows the week card and activity tiles, phones do not show the week', async ({
	page
}) => {
	const demo = seedDemoUser(server, 14);
	await login(page, demo);
	// The demo data is random per weekday and can leave today with no steps or gym, which hides the
	// tiles; give today steps so the test doesn't depend on the calendar. The demo user is on UTC.
	await seedDay(page, new Date().toISOString().slice(0, 10), { steps: 6000 });
	await page.reload();

	const jug = page.getByRole('img', { name: /kcal (plan|goal) eaten/ });
	await expect(jug).toBeVisible();
	await expect(page.getByRole('heading', { name: 'This week' })).toHaveCount(0);

	await page.setViewportSize({ width: 1280, height: 900 });
	const week = page.getByRole('region', { name: 'This week' });
	await expect(week).toBeVisible();
	await expect(week.locator('ol.bars > li.bar')).toHaveCount(7);
	await expect(week.locator('li.bar.today')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Sync steps' })).toHaveCount(0);

	const tiles = page.getByRole('region', { name: 'Activity' });
	await expect(tiles.getByRole('link')).toHaveCount(2);
	await tiles.getByRole('link').first().click();
	await expect(page).toHaveURL(/\/activity$/);
});
