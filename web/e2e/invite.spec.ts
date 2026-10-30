// SPDX-License-Identifier: AGPL-3.0-or-later
// Invites (PLAN.md §6, M7): an admin creates a one-time link, a second browser accepts it and
// lands on an empty Today, the used link is refused, a revoked invite is refused, and the Users
// section stays hidden from a non-admin. This file runs its own server and data dir.
import { expect, test, type Browser, type Page } from '@playwright/test';
import { USER_A, randomPort, startCupful, type CupfulServer } from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });

const port = randomPort();
test.use({ baseURL: `http://127.0.0.1:${port}` });

const timezone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;
/** Copy from `inviteErrorMessage` for a used, expired or unknown token. */
const GONE = 'This invite is used, has expired or is not valid. Ask the admin for a new link.';

let server: CupfulServer;

test.beforeAll(async () => {
	server = await startCupful(port);
});

test.afterAll(() => server?.stop());

async function signInAdmin(page: Page): Promise<void> {
	const res = await page.request.post('/api/auth/login', {
		headers: { 'X-Cupful': '1' },
		data: USER_A
	});
	expect(res.ok()).toBe(true);
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
}

async function openUsers(page: Page) {
	await page.getByRole('link', { name: 'Settings' }).click();
	const users = page.getByRole('region', { name: 'Users' });
	await expect(users).toBeVisible();
	return users;
}

/** Creates an invite and returns the one-time link shown in the Users card. */
async function createInvite(page: Page): Promise<string> {
	const users = await openUsers(page);
	await users.getByRole('button', { name: 'Create invite' }).click();
	const link = users.getByTestId('new-invite');
	await expect(link).toBeVisible();
	const url = (await link.textContent())?.trim() ?? '';
	expect(url).toMatch(/\/invite\?token=[\w-]+$/);
	return url;
}

/** Opens an invite link in a fresh browser context and submits the join form. */
async function join(
	browser: Browser,
	url: string,
	username: string
): Promise<{ page: Page; close: () => Promise<void> }> {
	const context = await browser.newContext({ baseURL: server.base });
	const page = await context.newPage();
	await page.goto(url);
	await expect(page.getByRole('heading', { name: 'Join Cupful', level: 1 })).toBeVisible();
	// The token is read once and replaced out of the address bar.
	expect(page.url()).toBe(`${server.base}/invite`);
	await page.getByLabel('Username').fill(username);
	await page.getByLabel('Password').fill('fresh password');
	await page.selectOption('#timezone', timezone());
	await page.getByRole('button', { name: 'Create account' }).click();
	return { page, close: () => context.close() };
}

test('an admin invite is accepted once and lands on an empty Today', async ({ page, browser }) => {
	await signInAdmin(page);
	const url = await createInvite(page);

	const joined = await join(browser, url, 'friend');
	await joined.page.waitForURL(`${server.base}/`);
	await expect(joined.page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(joined.page.getByText('Nothing logged yet. Describe what you ate above.')).toBeVisible();

	// A non-admin has no Users section, so no invite controls either.
	await joined.page.getByRole('link', { name: 'Settings' }).click();
	await expect(joined.page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
	await expect(joined.page.getByRole('region', { name: 'Users' })).toHaveCount(0);
	await expect(joined.page.getByRole('button', { name: 'Create invite' })).toHaveCount(0);
	await joined.close();

	// The same link cannot make a second account.
	const reused = await join(browser, url, 'someone-else');
	await expect(reused.page.getByRole('alert')).toHaveText(GONE);
	await reused.close();
});

test('a revoked invite link is refused', async ({ page, browser }) => {
	await signInAdmin(page);
	const users = await openUsers(page);
	await users.getByRole('button', { name: 'Create invite' }).click();
	const url = (await users.getByTestId('new-invite').textContent())?.trim() ?? '';
	expect(url).toMatch(/\/invite\?token=[\w-]+$/);
	await users.getByRole('button', { name: 'Done' }).click();

	// Only this pending invite is listed (the first test's invite is used, not pending).
	const pending = users.getByRole('listitem').filter({ hasText: 'Invite' });
	await expect(pending).toHaveCount(1);
	await pending.getByRole('button', { name: /^Revoke/ }).click();
	await users.getByRole('button', { name: 'Revoke invite' }).click();
	await expect(users.getByRole('status')).toHaveText('Invite revoked.');
	await expect(users.getByText('No pending invites. Create one to let someone join.')).toBeVisible();

	const revoked = await join(browser, url, 'late-joiner');
	await expect(revoked.page.getByRole('alert')).toHaveText(GONE);
	await revoked.close();
});
