// SPDX-License-Identifier: AGPL-3.0-or-later
// E2E against the real Rust server (debug build, which serves web/build from disk) with a fresh
// data directory, product lookups off and no AI provider.
import { defineConfig, devices } from '@playwright/test';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Workers re-load this file; the env carries the first load's choices to them.
// ponytail: random port, may collide with a busy one; probe for a free port if that ever bites.
process.env.CUPFUL_E2E_PORT ??= String(20000 + Math.floor(Math.random() * 20000));
process.env.CUPFUL_E2E_DATA ??= mkdtempSync(join(tmpdir(), 'cupful-e2e-'));
const port = process.env.CUPFUL_E2E_PORT;

export default defineConfig({
	testDir: 'e2e',
	forbidOnly: !!process.env.CI,
	retries: 0, // a retry would meet the database the first attempt already set up
	workers: 1, // one server, one database: tests share the first-run state
	reporter: process.env.CI ? 'github' : 'list',
	use: {
		baseURL: `http://127.0.0.1:${port}`,
		trace: 'retain-on-failure'
	},
	projects: [
		{
			name: 'phone',
			use: { ...devices['Desktop Chrome'], viewport: { width: 360, height: 740 }, hasTouch: true }
		}
	],
	webServer: {
		command: 'npm run build && cd ../server && cargo run -- serve',
		url: `http://127.0.0.1:${port}/api/health`,
		timeout: 600_000, // first cargo build
		reuseExistingServer: false,
		stdout: 'ignore',
		stderr: 'pipe',
		env: {
			CUPFUL_BIND: `127.0.0.1:${port}`,
			CUPFUL_DATA_DIR: process.env.CUPFUL_E2E_DATA,
			CUPFUL_SECURE_COOKIES: 'false',
			CUPFUL_PRODUCT_LOOKUP: 'off',
			// Empty counts as unset: no provider seeding or key from the developer's shell.
			CUPFUL_PROVIDER_BASE_URL: '',
			CUPFUL_SECRET_KEY: ''
		}
	}
});
