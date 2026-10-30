// SPDX-License-Identifier: AGPL-3.0-or-later
/// <reference types="vitest/config" />
import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
	version: string;
};

/**
 * A deterministic build name: `CUPFUL_BUILD_ID` when set, else `<version>+<short git SHA>`, else
 * the package version. The service worker caches under this name, and Settings shows it.
 */
function buildName(): string {
	if (process.env.CUPFUL_BUILD_ID) return process.env.CUPFUL_BUILD_ID;
	try {
		const sha = execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] })
			.toString()
			.trim();
		if (sha) return `${pkg.version}+${sha}`;
	} catch {
		// not a git checkout; fall back to the version alone
	}
	return pkg.version;
}

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// SPA: the Rust server serves index.html for any unknown non-API path.
			adapter: adapter({ fallback: 'index.html' }),
			version: { name: buildName() }
		})
	],
	server: {
		proxy: { '/api': 'http://127.0.0.1:8080' }
	},
	test: {
		include: ['src/**/*.test.ts'],
		setupFiles: ['src/test-setup.ts']
	}
});
