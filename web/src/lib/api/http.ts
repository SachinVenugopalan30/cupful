// SPDX-License-Identifier: AGPL-3.0-or-later
// Fetch plumbing: JSON requests, the allowed methods, and cached reads for an active user.

import { ApiError, parseError } from './errors.ts';
import { CacheHttpError, cachedGetJson } from '../offline/cache.ts';
import { connection } from '../offline/state.svelte.ts';

/**
 * Session gone: back to login. Auth routes answer 401 for a wrong password; their page shows that.
 */
function leaveIfSessionExpired(status: number, path: string): void {
	if (status === 401 && !path.startsWith('/api/auth/') && typeof location !== 'undefined') {
		location.assign('/login');
	}
}

/** Sends the request and returns the response when it's 2xx; throws an ApiError otherwise. */
export async function send(
	method: string,
	path: string,
	body?: unknown,
	contentType = 'application/json'
): Promise<Response> {
	const headers = new Headers();
	if (method !== 'GET') headers.set('X-Cupful', '1');
	if (body !== undefined && !(body instanceof FormData)) headers.set('Content-Type', contentType);
	const payload =
		body === undefined || body instanceof FormData || typeof body === 'string'
			? (body as BodyInit | undefined)
			: JSON.stringify(body);
	const res = await fetch(path, {
		method,
		headers,
		credentials: 'same-origin',
		body: payload
	});
	if (!res.ok) {
		leaveIfSessionExpired(res.status, path);
		throw await parseError(res);
	}
	return res;
}

/** Sends the request and parses the JSON answer. */
export async function request<T>(
	method: string,
	path: string,
	body?: unknown,
	contentType = 'application/json'
): Promise<T> {
	const res = await send(method, path, body, contentType);
	return res.json() as Promise<T>;
}

export const api = {
	get: <T>(path: string) => request<T>('GET', path),
	post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
	put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
	delete: <T>(path: string) => request<T>('DELETE', path),
	/** Multipart POST; the browser sets the boundary. */
	postForm: <T>(path: string, form: FormData) => request<T>('POST', path, form)
};

/**
 * GET a read endpoint through the offline cache once a user is active: network first, cached body
 * when offline. Cache errors become ApiErrors, and a 401 still sends the user to /login.
 */
export async function cachedGet<T>(path: string): Promise<T> {
	const userId = connection.activeUserId;
	if (userId === null) return api.get<T>(path);
	try {
		return await cachedGetJson<T>(path, userId);
	} catch (err) {
		if (err instanceof CacheHttpError) {
			leaveIfSessionExpired(err.status, path);
			throw new ApiError(err.status, err.message);
		}
		throw err;
	}
}

export const enc = encodeURIComponent;
