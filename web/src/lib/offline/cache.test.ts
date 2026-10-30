// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	CacheHttpError,
	cachedGetJson,
	clearUserCache,
	getCachedBody,
	isCacheable,
	putCachedBody
} from './cache.ts';

function json(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'Content-Type': 'application/json' }
	});
}

afterEach(async () => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	await clearUserCache(9101);
	await clearUserCache(9102);
});

describe('isCacheable', () => {
	it('allows the read endpoints', () => {
		expect(isCacheable('/api/meals?date=2026-10-29')).toBe(true);
		expect(isCacheable('/api/days/2026-10-29')).toBe(true);
		expect(isCacheable('/api/days?from=2026-10-01&to=2026-10-29')).toBe(true);
		expect(isCacheable('/api/budget/2026-10-29')).toBe(true);
		expect(isCacheable('/api/trends?from=2026-10-01&to=2026-10-29')).toBe(true);
		expect(isCacheable('/api/products')).toBe(true);
		expect(isCacheable('/api/profile')).toBe(true);
	});

	it('refuses configuration and auth', () => {
		expect(isCacheable('/api/auth/state')).toBe(false);
		expect(isCacheable('/api/providers')).toBe(false);
		expect(isCacheable('/api/tokens')).toBe(false);
		expect(isCacheable('/api/products?q=milk')).toBe(false);
		expect(isCacheable('/api/profile/versions')).toBe(false);
		expect(isCacheable('/api/google/status')).toBe(false);
		expect(isCacheable('/api/health')).toBe(false);
	});
});

describe('cachedGetJson', () => {
	it('stores a 2xx body under the user key', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => json({ date: '2026-10-29', steps: 1234 }))
		);

		const body = await cachedGetJson<{ steps: number }>('/api/days/2026-10-29', 9101);
		expect(body.steps).toBe(1234);
		expect(await getCachedBody(9101, '/api/days/2026-10-29')).toEqual({
			date: '2026-10-29',
			steps: 1234
		});
	});

	it('falls back to the cache when the network fails', async () => {
		await putCachedBody(9101, '/api/days/2026-10-29', { steps: 777 });
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);

		expect(await cachedGetJson('/api/days/2026-10-29', 9101)).toEqual({ steps: 777 });
	});

	it('serves the cache without fetching when the browser is offline', async () => {
		await putCachedBody(9101, '/api/days/2026-10-29', { steps: 321 });
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		vi.stubGlobal('navigator', { onLine: false });

		expect(await cachedGetJson('/api/days/2026-10-29', 9101)).toEqual({ steps: 321 });
		expect(fetchMock).not.toHaveBeenCalled();
	});

	it('says the page is not saved yet when offline with no cache', async () => {
		vi.stubGlobal('fetch', vi.fn());
		vi.stubGlobal('navigator', { onLine: false });

		await expect(cachedGetJson('/api/days/2026-10-29', 9101)).rejects.toThrow(/offline/i);
	});

	it(
		'falls back to the cache after a hanging request',
		async () => {
			await putCachedBody(9101, '/api/days/2026-10-29', { steps: 654 });
			vi.stubGlobal(
				'fetch',
				vi.fn(
					(_url: string, init?: RequestInit) =>
						new Promise((_resolve, reject) => {
							init?.signal?.addEventListener('abort', () =>
								reject(new DOMException('Aborted', 'AbortError'))
							);
						})
				)
			);

			expect(await cachedGetJson('/api/days/2026-10-29', 9101)).toEqual({ steps: 654 });
		},
		8_000
	);

	it('falls back to the cache on a 5xx answer', async () => {
		await putCachedBody(9101, '/api/days/2026-10-29', { steps: 555 });
		vi.stubGlobal('fetch', vi.fn(async () => json({ error: 'boom' }, 503)));

		expect(await cachedGetJson('/api/days/2026-10-29', 9101)).toEqual({ steps: 555 });
	});

	it('throws CacheHttpError on 401 and never falls back', async () => {
		await putCachedBody(9101, '/api/days/2026-10-29', { steps: 999 });
		vi.stubGlobal('fetch', vi.fn(async () => json({ error: 'unauthorized' }, 401)));

		await expect(cachedGetJson('/api/days/2026-10-29', 9101)).rejects.toMatchObject({
			name: 'CacheHttpError',
			status: 401
		});
	});

	it('throws CacheHttpError on 404 without falling back', async () => {
		await putCachedBody(9101, '/api/days/2026-10-29', { steps: 1 });
		vi.stubGlobal('fetch', vi.fn(async () => json({ error: 'not found' }, 404)));

		await expect(cachedGetJson('/api/days/2026-10-29', 9101)).rejects.toBeInstanceOf(
			CacheHttpError
		);
	});

	it('does not cache non-allowlisted paths and fails without a fallback', async () => {
		const fetchMock = vi.fn(async () => json({ user: null }));
		vi.stubGlobal('fetch', fetchMock);
		expect(await cachedGetJson('/api/auth/state', 9101)).toEqual({ user: null });
		expect(await getCachedBody(9101, '/api/auth/state')).toBeUndefined();

		fetchMock.mockImplementationOnce(async () => {
			throw new TypeError('Failed to fetch');
		});
		await expect(cachedGetJson('/api/auth/state', 9101)).rejects.toThrow('Failed to fetch');
	});

	it('keeps cached bodies separate per user', async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(json({ steps: 1 }))
			.mockResolvedValueOnce(json({ steps: 2 }));
		vi.stubGlobal('fetch', fetchMock);

		await cachedGetJson('/api/days/2026-10-29', 9101);
		await cachedGetJson('/api/days/2026-10-29', 9102);

		expect(await getCachedBody(9101, '/api/days/2026-10-29')).toEqual({ steps: 1 });
		expect(await getCachedBody(9102, '/api/days/2026-10-29')).toEqual({ steps: 2 });
	});
});
