// SPDX-License-Identifier: AGPL-3.0-or-later
// Cupful's service worker: precache the app shell, serve navigation network-first, and serve
// hashed static assets cache-first. API reads are not cached here; the offline layer does that
// in IndexedDB so every response stays user-scoped.

/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

declare let self: ServiceWorkerGlobalScope;

import { assets, immutable } from '$app/manifest';
import { dev, version } from '$app/env';
import { asset } from '$app/paths';

/** `asset()` with a path that is not a static file (app shell, API prefix, build output). */
const at = (path: string) => asset(path as Parameters<typeof asset>[0]);

const CACHE = `cupful-${version}`;
const API_PREFIX = at('api/');
const INDEX = at('index.html');
const ROOT = at('');

const PRECACHE = Array.from(
	new Set([
		ROOT,
		INDEX,
		...immutable.filter((entry) => !entry.path.endsWith('.wasm')).map((entry) => at(entry.path)),
		// The barcode scanner is a ~925 KB wasm file most people never need. Leave it out of the
		// install list; the fetch handler caches it on first scan.
		...assets.filter((entry) => !entry.path.endsWith('.wasm')).map((entry) => asset(entry.path))
	])
);

self.addEventListener('install', (event) => {
	// Dev builds are never cached: `npm run dev` must always show the current source.
	if (dev) return;
	event.waitUntil(
		caches.open(CACHE).then((cache) => cache.addAll(PRECACHE))
	);
});

self.addEventListener('activate', (event) => {
	event.waitUntil(
		(async () => {
			const keys = await caches.keys();
			await Promise.all(
				keys
					.filter((key) => key.startsWith('cupful-') && key !== CACHE)
					.map((key) => caches.delete(key))
			);
			await self.clients.claim();
		})()
	);
});

self.addEventListener('message', (event) => {
	if (event.data?.type === 'SKIP_WAITING') void self.skipWaiting();
});

async function networkFirstNavigation(request: Request): Promise<Response> {
	try {
		return await fetch(request);
	} catch {
		const cache = await caches.open(CACHE);
		return (await cache.match(INDEX)) ?? (await cache.match(ROOT)) ?? Response.error();
	}
}

async function cacheFirst(request: Request, event: FetchEvent): Promise<Response> {
	const cache = await caches.open(CACHE);
	const cached = await cache.match(request);
	if (cached) {
		// Serve immediately, refresh in the background for the next visit.
		event.waitUntil(
			fetch(request)
				.then((res) => (res.ok ? cache.put(request, res.clone()) : undefined))
				.catch(() => undefined)
		);
		return cached;
	}

	const res = await fetch(request);
	if (res.ok) event.waitUntil(cache.put(request, res.clone()));
	return res;
}

self.addEventListener('fetch', (event) => {
	if (dev) return;

	const request = event.request;
	if (request.method !== 'GET') return;

	const url = new URL(request.url);
	if (url.origin !== self.location.origin) return;
	// API reads and writes are handled by the app (IndexedDB cache, write queue).
	if (url.pathname.startsWith(API_PREFIX)) return;

	if (request.mode === 'navigate') {
		event.respondWith(networkFirstNavigation(request));
		return;
	}

	event.respondWith(cacheFirst(request, event));
});
