// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	api,
	ApiError,
	errorMessage,
	getBudget,
	getDay,
	getMeals,
	getTrends,
	importCsv,
	parseError,
	products,
	profile,
	providers
} from './api';
import { CacheHttpError } from './offline/cache.ts';

const mocks = vi.hoisted(() => ({
	connection: { activeUserId: null as number | null },
	queueState: { ops: [] as unknown[] },
	cachedGetJson: vi.fn(),
	overlayMeals: vi.fn(),
	overlayDay: vi.fn()
}));

vi.mock('./offline/state.svelte.ts', () => ({
	connection: mocks.connection,
	queueState: mocks.queueState
}));
vi.mock('./offline/cache.ts', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./offline/cache.ts')>();
	return { ...actual, cachedGetJson: mocks.cachedGetJson };
});
vi.mock('./offline/overlay.ts', () => ({
	overlayMeals: mocks.overlayMeals,
	overlayDay: mocks.overlayDay
}));

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

	it('sends the user to login on 401, except from auth routes', async () => {
		const assign = vi.fn();
		vi.stubGlobal('location', { assign });

		stub(json({ error: 'session expired' }, 401));
		await expect(api.get('/api/meals')).rejects.toMatchObject({ status: 401 });
		expect(assign).toHaveBeenCalledWith('/login');

		assign.mockClear();
		stub(json({ error: 'wrong password' }, 401));
		await expect(api.post('/api/auth/login', {})).rejects.toMatchObject({ status: 401 });
		expect(assign).not.toHaveBeenCalled();
	});

	it('posts a CSV text body for import', async () => {
		const call = stub(json({ meals_created: 1 }));
		await importCsv('date,description\n2026-10-29,rice');
		const [url, init] = call();
		expect(url).toBe('/api/import/fuel-ledger');
		expect(new Headers(init.headers).get('Content-Type')).toBe('text/csv');
		expect(init.body).toContain('rice');
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
			const m = errorMessage(new ApiError(502, 'raw', code), 'estimate');
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
		const e = new TypeError('Failed to fetch');
		expect(errorMessage(e)).toMatch(/connection/i);
		expect(errorMessage(e, 'estimate')).toMatch(/Estimates need a connection/);
	});

	it('falls back to the server message', () => {
		expect(errorMessage(new ApiError(400, 'grams must be 0-2000'))).toBe('grams must be 0-2000');
	});
});

describe('cached reads', () => {
	const date = '2026-10-29';

	beforeEach(() => {
		mocks.connection.activeUserId = null;
		mocks.queueState.ops = [];
		for (const fn of [mocks.cachedGetJson, mocks.overlayMeals, mocks.overlayDay]) fn.mockReset();
	});

	afterEach(() => vi.unstubAllGlobals());

	it('gets meals through the cache and applies queued ops', async () => {
		const meals = [{ id: 'm1' }];
		const shown = [{ id: 'm1', description: 'optimistic' }];
		const ops = [{ op_id: 'o1' }];
		mocks.connection.activeUserId = 42;
		mocks.queueState.ops = ops;
		mocks.cachedGetJson.mockResolvedValue({ meals });
		mocks.overlayMeals.mockReturnValue(shown);

		expect(await getMeals(date)).toBe(shown);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith(`/api/meals?date=${date}`, 42);
		expect(mocks.overlayMeals).toHaveBeenCalledWith(meals, ops, date);
	});

	it('gets the day through the cache and applies queued patches', async () => {
		const serverDay = { date, steps: 1000, steps_source: 'manual', trained: [] };
		const shown = { ...serverDay, steps: 2500 };
		mocks.connection.activeUserId = 42;
		mocks.cachedGetJson.mockResolvedValue(serverDay);
		mocks.overlayDay.mockReturnValue(shown);

		expect(await getDay(date)).toBe(shown);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith(`/api/days/${date}`, 42);
		expect(mocks.overlayDay).toHaveBeenCalledWith(serverDay, mocks.queueState.ops, date);
	});

	it('uses the cache for budget, trends, profile and saved products', async () => {
		mocks.connection.activeUserId = 7;
		mocks.cachedGetJson.mockImplementation(async (path: string) => {
			if (path.startsWith('/api/trends'))
				return { days: [{ date }], averages: null, meal_mix: null };
			if (path === '/api/products') return { products: [{ food_id: 1 }] };
			return { date };
		});

		expect(await getBudget(date)).toEqual({ date });
		expect(await getTrends('2026-10-01', date)).toEqual({
			days: [{ date }],
			averages: null,
			meal_mix: null
		});
		expect(await profile.get()).toEqual({ date });
		expect(await products.list()).toEqual([{ food_id: 1 }]);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith(`/api/budget/${date}`, 7);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith(`/api/trends?from=2026-10-01&to=${date}`, 7);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith('/api/profile', 7);
		expect(mocks.cachedGetJson).toHaveBeenCalledWith('/api/products', 7);
	});

	it('maps a cached 401 to ApiError and sends the user to login', async () => {
		const assign = vi.fn();
		vi.stubGlobal('location', { assign });
		mocks.connection.activeUserId = 42;
		mocks.cachedGetJson.mockRejectedValue(
			new CacheHttpError(401, 'The server answered with an error (401).')
		);

		const err = await getBudget(date).catch((e: unknown) => e);
		expect(err).toBeInstanceOf(ApiError);
		expect(err).toMatchObject({ status: 401 });
		expect(assign).toHaveBeenCalledWith('/login');
	});

	it('leaves configuration reads on the plain API', async () => {
		const fetch = vi.fn(async () => json({ providers: [] }));
		vi.stubGlobal('fetch', fetch);
		mocks.connection.activeUserId = 42;

		expect(await providers.list()).toEqual([]);
		expect(fetch).toHaveBeenCalled();
		expect(mocks.cachedGetJson).not.toHaveBeenCalled();
	});

	it('does not consult the cache before a user is active', async () => {
		const fetch = vi.fn(async () => json({ date }));
		vi.stubGlobal('fetch', fetch);

		expect(await getBudget(date)).toEqual({ date });
		expect(fetch).toHaveBeenCalled();
		expect(mocks.cachedGetJson).not.toHaveBeenCalled();
	});
});
