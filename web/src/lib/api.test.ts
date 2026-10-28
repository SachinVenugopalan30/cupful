// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import { api, ApiError, errorMessage, parseError } from './api';

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

describe('parseError', () => {
	it('reads the server error body and code', async () => {
		const e = await parseError(json({ error: 'no AI provider is configured', code: 'no_provider' }, 409));
		expect(e).toBeInstanceOf(ApiError);
		expect(e).toMatchObject({ status: 409, code: 'no_provider', message: 'no AI provider is configured' });
	});

	it('copes with a body that is not JSON', async () => {
		const e = await parseError(new Response('<html>Bad Gateway</html>', { status: 502 }));
		expect(e.status).toBe(502);
		expect(e.code).toBeUndefined();
		expect(e.message).toMatch(/502/);
	});
});

describe('request', () => {
	afterEach(() => vi.unstubAllGlobals());

	function stub(res: Response) {
		const fetch = vi.fn(async () => res);
		vi.stubGlobal('fetch', fetch);
		return () => fetch.mock.calls[0] as unknown as [string, RequestInit];
	}

	it('GET sends no CSRF header and returns JSON', async () => {
		const call = stub(json({ status: 'ok' }));
		expect(await api.get('/api/health')).toEqual({ status: 'ok' });
		const [url, init] = call();
		expect(url).toBe('/api/health');
		expect(new Headers(init.headers).has('X-Cupful')).toBe(false);
		expect(init.credentials).toBe('same-origin');
	});

	it('mutating requests send JSON and X-Cupful: 1', async () => {
		const call = stub(json({ id: 1 }));
		await api.put('/api/profile', { a: 1 });
		const [, init] = call();
		const h = new Headers(init.headers);
		expect(init.method).toBe('PUT');
		expect(h.get('X-Cupful')).toBe('1');
		expect(h.get('Content-Type')).toBe('application/json');
		expect(init.body).toBe('{"a":1}');
	});

	it('form posts leave the content type to the browser', async () => {
		const call = stub(json({ items: [] }));
		const form = new FormData();
		form.set('description', 'rice');
		await api.postForm('/api/estimate', form);
		const [, init] = call();
		const h = new Headers(init.headers);
		expect(h.get('X-Cupful')).toBe('1');
		expect(h.has('Content-Type')).toBe(false);
		expect(init.body).toBe(form);
	});

	it('throws ApiError on a non-2xx answer', async () => {
		stub(json({ error: 'product not found', code: 'product_not_found' }, 404));
		await expect(api.get('/api/products/4006381333931')).rejects.toMatchObject({
			status: 404,
			code: 'product_not_found'
		});
	});
});

describe('errorMessage', () => {
	it('maps AI codes to plain language', () => {
		for (const code of [
			'no_provider',
			'timeout',
			'unreachable',
			'unparseable',
			'rate_limited',
			'daily_cap',
			'provider_auth'
		]) {
			const m = errorMessage(new ApiError(502, 'raw', code));
			expect(m).not.toBe('raw');
			expect(m.length).toBeGreaterThan(20);
		}
	});

	it('keeps the server detail for a generic provider error', () => {
		expect(errorMessage(new ApiError(502, 'model not found', 'provider_error'))).toMatch(
			/model not found/
		);
	});

	it('words rate limits by context', () => {
		const e = new ApiError(429, 'raw', 'rate_limited');
		expect(errorMessage(e, 'product')).toMatch(/label/);
		expect(errorMessage(e, 'estimate')).not.toMatch(/label/);
	});

	it('maps product codes', () => {
		expect(errorMessage(new ApiError(404, 'raw', 'product_not_found'))).toMatch(/label/);
		expect(errorMessage(new ApiError(502, 'raw', 'lookup_failed'))).toMatch(/label/);
	});

	it('explains network failures', () => {
		expect(errorMessage(new TypeError('Failed to fetch'))).toMatch(/connection/i);
	});

	it('falls back to the server message', () => {
		expect(errorMessage(new ApiError(400, 'grams must be 0-2000'))).toBe('grams must be 0-2000');
	});
});
