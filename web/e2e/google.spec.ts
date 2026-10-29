// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, test } from '@playwright/test';

// The e2e server has no Google client configured: the feature is invisible and its routes 404.
test('Google Health is hidden when the server has no Google client', async ({ page }) => {
	await page.goto('/');
	await page.waitForURL(/\/(setup|login)$/);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password').fill('correct horse');
	await page.getByRole('button', { name: page.url().endsWith('/setup') ? 'Create account' : 'Log in' }).click();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();

	expect(await (await page.request.get('/api/google')).json()).toEqual({ configured: false });
	expect((await page.request.get('/api/google/connect', { maxRedirects: 0 })).status()).toBe(404);

	await page.goto('/settings');
	await expect(page.getByRole('heading', { name: 'AI providers' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Google Health' })).toHaveCount(0);
});
