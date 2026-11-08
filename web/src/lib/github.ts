// SPDX-License-Identifier: AGPL-3.0-or-later
// The nav's GitHub link. The server owns the hourly cache; this keeps the link usable offline.

export const GITHUB_URL = 'https://github.com/SachinVenugopalan30/cupful';

export type GitHubInfo = { url: string; stars: number | null };

export async function getGitHubInfo(): Promise<GitHubInfo> {
	try {
		const response = await fetch('/api/github', { headers: { accept: 'application/json' } });
		if (!response.ok) return { url: GITHUB_URL, stars: null };
		const data = (await response.json()) as { url?: unknown; stars?: unknown };
		return {
			url: typeof data.url === 'string' ? data.url : GITHUB_URL,
			stars: Number.isInteger(data.stars) ? (data.stars as number) : null
		};
	} catch {
		return { url: GITHUB_URL, stars: null };
	}
}

export const compactStars = (stars: number) =>
	new Intl.NumberFormat(undefined, { notation: 'compact' }).format(stars);
