// SPDX-License-Identifier: AGPL-3.0-or-later
// Tailscale identity mode (PLAN.md §10, M7, D4 to D7). With mode on, a header from the sidecar
// logs an anonymous visitor in without a password and invites are hidden. With mode off, the same
// forged header changes nothing. Each server gets its own data dir.
import { expect, test } from '@playwright/test';
import { USER_A, randomPort, startCupful, type CupfulServer } from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });

let identity: CupfulServer;
let plain: CupfulServer;

test.beforeAll(async () => {
	identity = await startCupful(randomPort(), { CUPFUL_TRUST_TAILSCALE: 'true' });
	// Setup on both happens over Node fetch with no header, so no identity is mapped.
	plain = await startCupful(randomPort());
});

test.afterAll(() => {
	identity?.stop();
	plain?.stop();
});

test('the header signs sam in without a password and hides invites', async ({ browser }) => {
	const context = await browser.newContext({
		baseURL: identity.base,
		extraHTTPHeaders: { 'Tailscale-User-Login': 'sam@example.com' }
	});
	const page = await context.newPage();
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();

	const state = (await (await page.request.get('/api/auth/state')).json()) as {
		user: { username: string; is_admin: boolean } | null;
		identity_mode: string | null;
	};
	expect(state.user?.username).toBe('sam');
	expect(state.user?.is_admin).toBe(false);
	expect(state.identity_mode).toBe('tailscale');

	// Settings shows the account the header created; the Users section is admin-only.
	await page.getByRole('link', { name: 'Settings' }).click();
	await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
	await expect(page.getByText('Logged in as')).toContainText('sam');
	await expect(page.getByRole('region', { name: 'Users' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Create invite' })).toHaveCount(0);
	await context.close();

	// An admin sees the mode notice and no invite controls.
	const adminContext = await browser.newContext({ baseURL: identity.base });
	const admin = await adminContext.newPage();
	const login = await admin.request.post('/api/auth/login', {
		headers: { 'X-Cupful': '1' },
		data: USER_A
	});
	expect(login.ok()).toBe(true);
	await admin.goto('/settings');
	const users = admin.getByRole('region', { name: 'Users' });
	await expect(users).toBeVisible();
	await expect(users.getByText(/This server signs people in with Tailscale\./)).toBeVisible();
	await expect(users.getByRole('button', { name: 'Create invite' })).toHaveCount(0);
	await expect(users.getByText('No pending invites. Create one to let someone join.')).toHaveCount(0);
	await adminContext.close();
});

test('the same header does nothing when identity mode is off', async ({ browser }) => {
	const context = await browser.newContext({
		baseURL: plain.base,
		extraHTTPHeaders: { 'Tailscale-User-Login': 'sam@example.com' }
	});
	const page = await context.newPage();
	await page.goto('/');
	await page.waitForURL(/\/login$/);
	await expect(page.getByRole('heading', { name: 'Log in', level: 1 })).toBeVisible();

	const state = (await (await page.request.get('/api/auth/state')).json()) as {
		user: unknown;
		identity_mode: unknown;
	};
	expect(state.user).toBeNull();
	expect(state.identity_mode).toBeNull();
	await context.close();
});
