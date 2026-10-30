// SPDX-License-Identifier: AGPL-3.0-or-later
// User-scoped API read cache: network first, IndexedDB fallback. Only the small allowlist of
// read endpoints is cacheable, never auth, providers, tokens or other configuration.

import {
	cacheKey,
	deleteCacheEntry,
	getCacheEntry,
	getDb,
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

/**
 * True for the read endpoints that are safe to keep offline. Everything else (auth, providers,
 * ingest tokens, aliases, Google/health settings) is online-only.
 */
export function isCacheable(path: string): boolean {
	if (path.includes('#')) path = path.slice(0, path.indexOf('#'));
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

async function bodyOf(res: Response): Promise<unknown> {
	return res.json();
}

/**
 * GET JSON, network first. A 2xx answer is stored for this user; a network error or 5xx falls back
 * to the cached body when there is one. Client errors throw `CacheHttpError` without falling back.
 */
export async function cachedGetJson<T>(path: string, userId: number): Promise<T> {
	const cacheable = isCacheable(path);
	let networkError: unknown;

	try {
		const res = await fetch(path, {
			method: 'GET',
			credentials: 'same-origin',
			headers: { Accept: 'application/json' }
		});
		if (res.ok) {
			const body = await bodyOf(res);
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
	}

	if (cacheable) {
		const cached = await getCachedBody<T>(userId, path);
		if (cached !== undefined) return cached;
	}
	throw networkError ?? new Error('The request failed and nothing was cached.');
}

async function entry(userId: number, path: string): Promise<CacheEntry | undefined> {
	return getCacheEntry(userId, path);
}

/** The cached body for this user and path, or undefined. */
export async function getCachedBody<T>(userId: number, path: string): Promise<T | undefined> {
	const found = await entry(userId, path);
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

export async function deleteCachedBody(userId: number, path: string): Promise<void> {
	await deleteCacheEntry(userId, path);
}

/** Drops every cached API body for one user, leaving their queue and other accounts alone. */
export async function clearUserCache(userId: number): Promise<void> {
	const db = await getDb();
	const tx = db.transaction('cache', 'readwrite');
	const keys = await tx.store.index('by_user').getAllKeys(userId);
	for (const key of keys) await tx.store.delete(key);
	await tx.done;
}
