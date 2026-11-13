// SPDX-License-Identifier: AGPL-3.0-or-later
// Accessibility: axe checks every main screen, the Review sheet and Login against WCAG 2.1 A and
// AA, in light and dark, on a phone and on a desktop. A violation fails with its rule, its help
// text and the elements it hit, so the report says what to fix.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { login, randomPort, seedDemoUser, startCupful, type CupfulServer } from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });

const port = randomPort();
test.use({ baseURL: `http://127.0.0.1:${port}` });

let server: CupfulServer;
let demo: { username: string; password: string };

test.beforeAll(async () => {
	server = await startCupful(port);
	demo = seedDemoUser(server, 30);
});

test.afterAll(() => server?.stop());

/** Applies a theme through localStorage (the app reads it before first paint), then reloads. */
async function setTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	await page.evaluate((value) => localStorage.setItem('cupful-theme', value), theme);
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
}

/** Runs axe on the page as it is now and fails with a readable list of violations. */
async function expectAccessible(page: Page, where: string): Promise<void> {
	const { violations } = await new AxeBuilder({ page })
		.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
		.analyze();
	const report = violations.map(
		(v) => `${v.id} (${v.impact}): ${v.help}\n  ${v.nodes.map((n) => n.target.join(' ')).join('\n  ')}`
	);
	expect(report, `${where}\n${report.join('\n')}`).toEqual([]);
}

const SCREENS: [path: string, heading: string][] = [
	['/', 'Today'],
	['/activity', 'Activity'],
	['/trends', 'Trends'],
	['/settings', 'Settings'],
	['/profile', 'Profile']
];

for (const [width, label] of [
	[360, 'phone'],
	[1280, 'desktop']
] as const) {
	for (const theme of ['light', 'dark'] as const) {
		test(`screens pass axe on a ${label} in ${theme}`, async ({ page }) => {
			await page.setViewportSize({ width, height: 900 });
			await login(page, demo);
			await setTheme(page, theme);
			// Motion finishes instantly so axe sees the settled colours, not a mid-fade frame.
			await page.emulateMedia({ reducedMotion: 'reduce' });
			for (const [path, heading] of SCREENS) {
				await page.goto(path);
				await expect(page.getByRole('heading', { name: heading, level: 1 })).toBeVisible();
				await page.waitForLoadState('networkidle');
				await expectAccessible(page, `${path} on a ${label} in ${theme}`);
			}

			// The Review sheet, opened by hand from Today.
			await page.goto('/');
			await page.getByRole('button', { name: 'Enter by hand' }).click();
			await expect(page.locator('dialog[open]')).toBeVisible();
			await expectAccessible(page, `Review on a ${label} in ${theme}`);
		});
	}
}

test('Login passes axe in light and dark', async ({ browser }) => {
	const context = await browser.newContext({ baseURL: `http://127.0.0.1:${port}`, reducedMotion: 'reduce' });
	const page = await context.newPage();
	await page.goto('/login');
	for (const theme of ['light', 'dark'] as const) {
		await setTheme(page, theme);
		await expect(page.getByRole('button', { name: /log in/i })).toBeVisible();
		await expectAccessible(page, `Login in ${theme}`);
	}
	await context.close();
});
