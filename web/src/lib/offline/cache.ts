// SPDX-License-Identifier: AGPL-3.0-or-later
// User-scoped API read cache: network first, IndexedDB fallback. Only the small allowlist of
// read endpoints is cacheable, never auth, providers, tokens or other configuration.

import {
	cacheKey,
	getCacheEntry,
	putCacheEntry,
	type CacheEntry
} from './db.ts';

/** A non-retryable HTTP answer. The cache never substitutes for 401/403/404. */
export class CacheHttpError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'CacheHttpError';
	}
}

const CACHEABLE_EXACT = ['/api/products', '/api/profile'];
/** A dead connection can leave `fetch` hanging; fall back to the cache after this long. */
const NETWORK_TIMEOUT = 4_000;

/**
 * True for the read endpoints that are safe to keep offline. Everything else (auth, providers,
 * ingest tokens, aliases, Google/health settings) is online-only.
 */
export function isCacheable(path: string): boolean {
	const queryAt = path.indexOf('?');
	const route = queryAt === -1 ? path : path.slice(0, queryAt);
	const hasQuery = queryAt !== -1;

	if (route === '/api/meals') return hasQuery;
	if (route === '/api/days') return true; // `/api/days?from=…` range reads
	if (route.startsWith('/api/days/')) return true;
	if (route.startsWith('/api/budget/')) return true;
	if (route === '/api/trends') return hasQuery;
	return CACHEABLE_EXACT.includes(path);
}

/**
 * GET JSON, network first. A 2xx answer is stored for this user; a network error or 5xx falls back
 * to the cached body when there is one. Client errors throw `CacheHttpError` without falling back.
 */
export async function cachedGetJson<T>(path: string, userId: number): Promise<T> {
	const cacheable = isCacheable(path);
	let networkError: unknown;

	// Known-offline: don't wait for a fetch that cannot succeed. This is what makes Activity and
	// Trends open instantly from a cold start in airplane mode.
	if (cacheable && typeof navigator !== 'undefined' && navigator.onLine === false) {
		const cached = await getCachedBody<T>(userId, path);
		if (cached !== undefined) return cached;
		throw new Error('You’re offline and this page isn’t saved on this device yet.');
	}

	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), NETWORK_TIMEOUT);
	try {
		const res = await fetch(path, {
			method: 'GET',
			cache: 'no-store',
			credentials: 'same-origin',
			headers: { Accept: 'application/json' },
			signal: controller.signal
		});
		if (res.ok) {
			const body = await res.json();
			if (cacheable) await putCachedBody(userId, path, body);
			return body as T;
		}
		if (res.status < 500) {
			throw new CacheHttpError(res.status, `The server answered with an error (${res.status}).`);
		}
		networkError = new CacheHttpError(res.status, `The server answered with an error (${res.status}).`);
	} catch (err) {
		if (err instanceof CacheHttpError) throw err;
		networkError = err;
	} finally {
		clearTimeout(timer);
	}

	if (cacheable) {
		const cached = await getCachedBody<T>(userId, path);
		if (cached !== undefined) return cached;
	}
	throw networkError ?? new Error('The request failed and nothing was cached.');
}

/** The cached body for this user and path, or undefined. */
export async function getCachedBody<T>(userId: number, path: string): Promise<T | undefined> {
	const found: CacheEntry | undefined = await getCacheEntry(userId, path);
	return found?.body as T | undefined;
}

/** Stores one parsed JSON body under this user's key. */
export async function putCachedBody(
	userId: number,
	path: string,
	body: unknown
): Promise<void> {
	await putCacheEntry({
		key: cacheKey(userId, path),
		user_id: userId,
		path,
		body,
		saved_at: Date.now()
	});
}

/** Drops every cached API body for one user, leaving their queue and other accounts alone. */
export { clearCacheForUser as clearUserCache } from './db.ts';
