// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import { compactStars, getGitHubInfo, GITHUB_URL } from './github';

function reply(body: unknown, ok = true) {
	vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok, json: async () => body }));
}

afterEach(() => vi.unstubAllGlobals());

describe('getGitHubInfo', () => {
	it('uses the server url and star count', async () => {
		reply({ url: 'https://github.com/example/repo', stars: 7 });
		await expect(getGitHubInfo()).resolves.toEqual({ url: 'https://github.com/example/repo', stars: 7 });
		expect(fetch).toHaveBeenCalledWith('/api/github', { headers: { accept: 'application/json' } });
	});

	it('keeps the link and hides a null count', async () => {
		reply({ url: GITHUB_URL, stars: null });
		await expect(getGitHubInfo()).resolves.toEqual({ url: GITHUB_URL, stars: null });
	});

	it('falls back to the repo when the request fails', async () => {
		reply({}, false);
		await expect(getGitHubInfo()).resolves.toEqual({ url: GITHUB_URL, stars: null });
	});

	it('falls back when the network throws', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
		await expect(getGitHubInfo()).resolves.toEqual({ url: GITHUB_URL, stars: null });
	});

	it('rejects a non-integer count', async () => {
		reply({ url: GITHUB_URL, stars: 7.5 });
		await expect(getGitHubInfo()).resolves.toEqual({ url: GITHUB_URL, stars: null });
	});
});

describe('compactStars', () => {
	it('keeps small counts plain', () => {
		expect(compactStars(0)).toBe('0');
		expect(compactStars(7)).toBe('7');
	});
});
