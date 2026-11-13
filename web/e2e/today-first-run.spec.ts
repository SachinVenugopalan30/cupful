// SPDX-License-Identifier: AGPL-3.0-or-later
// Before a profile exists the budget read answers 409. Today shows an empty jug, says what to do and
// links to /profile, and logging keeps working. Its own server: no other spec has saved a profile.
import { expect, test } from '@playwright/test';
import { login, randomPort, startCupful, type CupfulServer } from './helpers/server';

const port = randomPort();
let server: CupfulServer;

test.use({ baseURL: `http://127.0.0.1:${port}` });

test.beforeAll(async () => {
	server = await startCupful(port);
});
test.afterAll(() => server.stop());

test('without a profile Today shows an empty jug and a way to the profile', async ({ page }) => {
	await login(page);
	const budget = page.getByRole('region', { name: 'Budget' });
	await expect(budget.getByText('Save your profile to see your daily plan.')).toBeVisible();
	await expect(budget.getByRole('img', { name: /Empty jug/ })).toBeVisible();
	await expect(budget.getByRole('alert')).toHaveCount(0);
	// No activity yet and no profile: no tiles for a thing nobody tracks.
	await expect(page.getByRole('region', { name: 'Activity' })).toHaveCount(0);
	// The composer is still there.
	await expect(page.getByRole('heading', { name: 'Log a meal' })).toBeVisible();

	await budget.getByRole('link', { name: 'Set up your profile' }).click();
	await expect(page).toHaveURL(/\/profile$/);
});
