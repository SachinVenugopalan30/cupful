// SPDX-License-Identifier: AGPL-3.0-or-later
// IndexedDB storage for the offline layer: the write queue, API read cache and small metadata.
// Every entry is user-scoped, and every function is safe to call during SSR (it no-ops).

import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import type { QueuedOp } from './types.ts';

export type CacheEntry = {
	/** `<user_id>:<path+query>` */
	key: string;
	user_id: number;
	/** The request path, including its query string. */
	path: string;
	/** The parsed JSON body, exactly as the server sent it. */
	body: unknown;
	/** Unix milliseconds. */
	saved_at: number;
};

export type MetaEntry = { key: string; value: unknown };

interface CupfulDB extends DBSchema {
	queue: {
		key: string;
		value: QueuedOp;
		indexes: { by_user_created: [number, number, string] };
	};
	cache: {
		key: string;
		value: CacheEntry;
		indexes: { by_user: number };
	};
	meta: {
		key: string;
		value: MetaEntry;
	};
}

export const DB_NAME = 'cupful-offline';
export const DB_VERSION = 1;

function hasIndexedDB(): boolean {
	return typeof indexedDB !== 'undefined';
}

let dbPromise: Promise<IDBPDatabase<CupfulDB>> | null = null;

/** Opens (once) the offline database: `queue`, `cache` and `meta`. */
export function getDb(): Promise<IDBPDatabase<CupfulDB>> {
	if (!hasIndexedDB()) {
		return Promise.reject(new Error('IndexedDB is not available in this environment.'));
	}
	dbPromise ??= openDB<CupfulDB>(DB_NAME, DB_VERSION, {
		upgrade(db) {
			const queue = db.createObjectStore('queue', { keyPath: 'op_id' });
			queue.createIndex('by_user_created', ['user_id', 'queued_at', 'op_id']);

			const cache = db.createObjectStore('cache', { keyPath: 'key' });
			cache.createIndex('by_user', 'user_id');

			db.createObjectStore('meta', { keyPath: 'key' });
		}
	});
	return dbPromise;
}

/** The cache key for one user's request. */
export function cacheKey(userId: number, path: string): string {
	return `${userId}:${path}`;
}

/** The range covering one user's queue entries, in creation order. */
function userQueueRange(userId: number): IDBKeyRange {
	return IDBKeyRange.bound(
		[userId, 0],
		[userId, Number.MAX_SAFE_INTEGER, '\uffff'],
		false,
		false
	);
}

// ---- queue ----

export async function putQueueItem(item: QueuedOp): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).put('queue', item);
}

export async function deleteQueueItem(opId: string): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).delete('queue', opId);
}

/** One user's queued ops, oldest first. */
export async function allQueueForUser(userId: number): Promise<QueuedOp[]> {
	if (!hasIndexedDB()) return [];
	return (await getDb()).getAllFromIndex('queue', 'by_user_created', userQueueRange(userId));
}

// ---- API read cache ----

// ponytail: no eviction or byte budget, logout is the only cleanup; add a per-user LRU or a quota
// check when long-lived accounts fill a phone's storage.
export async function putCacheEntry(entry: CacheEntry): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).put('cache', entry);
}

export async function getCacheEntry(
	userId: number,
	path: string
): Promise<CacheEntry | undefined> {
	if (!hasIndexedDB()) return undefined;
	return (await getDb()).get('cache', cacheKey(userId, path));
}

export async function deleteCacheEntry(userId: number, path: string): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).delete('cache', cacheKey(userId, path));
}

// ---- meta ----

export async function getMeta<T = unknown>(key: string): Promise<T | undefined> {
	if (!hasIndexedDB()) return undefined;
	const entry = await (await getDb()).get('meta', key);
	return entry?.value as T | undefined;
}

export async function putMeta(key: string, value: unknown): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).put('meta', { key, value });
}

export async function deleteMeta(key: string): Promise<void> {
	if (!hasIndexedDB()) return;
	await (await getDb()).delete('meta', key);
}

// ---- account isolation ----

/** Removes one user's queued ops and cached API bodies. Other users are untouched. */
export async function clearUserData(userId: number): Promise<void> {
	if (!hasIndexedDB()) return;
	const db = await getDb();

	const queueTx = db.transaction('queue', 'readwrite');
	const queueKeys = await queueTx.store.index('by_user_created').getAllKeys(userQueueRange(userId));
	for (const key of queueKeys) await queueTx.store.delete(key);
	await queueTx.done;

	const cacheTx = db.transaction('cache', 'readwrite');
	const cacheKeys = await cacheTx.store.index('by_user').getAllKeys(userId);
	for (const key of cacheKeys) await cacheTx.store.delete(key);
	await cacheTx.done;
}
