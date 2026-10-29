// SPDX-License-Identifier: AGPL-3.0-or-later
// Same-origin JSON client for the Cupful API, with types matching the server's JSON.
// Logged data (meals, days) is written only through `sendOps` in `#lib/sync`; everything here is
// reads and online-only config REST.

import type { WeightUnit, HeightUnit } from './format';

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** Stable machine-readable code, when the server sends one (AI and product errors). */
		public code?: string
	) {
		super(message);
		this.name = 'ApiError';
	}
}

/** Turns a non-2xx response into an ApiError from its `{"error", "code"?}` body. */
export async function parseError(res: Response): Promise<ApiError> {
	try {
		const body = await res.json();
		if (typeof body?.error === 'string') return new ApiError(res.status, body.error, body.code);
	} catch {
		// not JSON (a proxy's error page, say)
	}
	return new ApiError(res.status, `The server answered with an error (${res.status}).`);
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
	const headers = new Headers();
	if (method !== 'GET') headers.set('X-Cupful', '1');
	if (body !== undefined && !(body instanceof FormData)) headers.set('Content-Type', 'application/json');
	const res = await fetch(path, {
		method,
		headers,
		credentials: 'same-origin',
		body: body === undefined || body instanceof FormData ? body : JSON.stringify(body)
	});
	if (!res.ok) {
		// Session gone: back to login. Auth routes answer 401 for a wrong password; the page shows that.
		if (res.status === 401 && !path.startsWith('/api/auth/') && typeof location !== 'undefined') {
			location.assign('/login');
		}
		throw await parseError(res);
	}
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

// ---- types (server JSON) ----

export type Health = { status: string; version: string; source_url: string };

export type AuthUser = { id: number; username: string; is_admin: boolean };
export type AuthState = { needs_setup: boolean; user: AuthUser | null };

export type Sex = 'male' | 'female';
export type ProfileVersion = {
	effective_from: string;
	sex: Sex;
	birth_year: number;
	height_cm: number;
	start_weight_kg: number;
	goal_kg: number | null;
	pace_kg_per_week: number;
	typical_steps: number;
};
export type Profile = {
	timezone: string;
	weight_unit: WeightUnit;
	height_unit: HeightUnit;
	/** The version in effect today; `null` until the profile is first saved. */
	version: ProfileVersion | null;
};
/** PUT /api/profile body: units, timezone, and a new version effective from today. */
export type ProfileIn = Omit<Profile, 'version'> & Omit<ProfileVersion, 'effective_from'>;

export type BudgetFlag =
	| 'provisional'
	| 'deficit_capped'
	| 'floor_applied'
	| 'floor_above_maintenance'
	| 'macros_overcommitted';
/** GET /api/budget/{date}. kcal fields are integers, grams one decimal. 409 until a profile exists. */
export type Budget = {
	date: string;
	age: number;
	weight_kg: number;
	steps: number;
	met: number;
	bmr: number;
	base: number;
	steps_kcal: number;
	gym_kcal: number;
	maintenance: number;
	deficit: number;
	floor: number;
	budget: number;
	protein_g: number;
	fat_g: number;
	carbs_g: number;
	flags: BudgetFlag[];
};

export type ActivitySource = 'manual' | 'shortcut' | 'import' | 'google';
export type Day = {
	date: string;
	steps: number | null;
	steps_source: ActivitySource | null;
	gym_minutes: number | null;
	gym_source: ActivitySource | null;
	trained: string[];
	weight_kg: number | null;
};

export type MealType = 'breakfast' | 'lunch' | 'dinner' | 'snack';
export type MealSource = 'ai' | 'manual' | 'repeat' | 'import';
export type Confidence = 'low' | 'medium' | 'high';
export type Origin = 'food_db' | 'ai_estimate' | 'manual';
export type Macros = { kcal: number; protein_g: number; carbs_g: number; fat_g: number };

export type MealItem = Macros & {
	id: string;
	name: string;
	portion_text: string;
	grams: number | null;
	food_id: number | null;
	origin: Origin;
};
/** GET /api/meals?date= item. Values are stored already multiplied by `scale`. */
export type Meal = {
	id: string;
	date: string;
	meal_type: MealType;
	description: string;
	scale: number;
	assumptions: string | null;
	confidence: Confidence | null;
	source: MealSource;
	provider_id: number | null;
	/** Unix milliseconds. */
	updated_at: number;
	items: MealItem[];
	totals: Macros;
};

export type Candidate = Macros & { food_id: number; name: string };
/** One estimated item. Null macros: nothing usable was found; the user fills them in. */
export type EstimateItem = {
	name: string;
	portion_text: string;
	grams: number;
	kcal: number | null;
	protein_g: number | null;
	carbs_g: number | null;
	fat_g: number | null;
	origin: Origin;
	food_id: number | null;
	food_name: string | null;
	match: 'matched' | 'ambiguous' | 'unmatched';
	/** For ambiguous items, alternatives with values for this item's grams, best first. */
	candidates: Candidate[];
};
export type Estimate = {
	items: EstimateItem[];
	assumptions: string | null;
	confidence: Confidence | null;
	provider_id: number | null;
	/** Items the server dropped, in words. */
	warnings: string[];
};

/** A food row, per 100 g (POST /api/foods answer). */
export type Food = Macros & {
	id: number;
	name: string;
	source: string;
	is_prepared: boolean;
	default_portion_g: number | null;
};
export type NewFood = Macros & { name: string; is_prepared?: boolean; default_portion_g?: number | null };
export type Alias = { id: number; alias: string; food_id: number };
/** GET /api/aliases: the caller's own phrases, newest first, with the food each points at. */
export type AliasRow = {
	id: number;
	alias: string;
	food_id: number;
	food_name: string;
	brand: string | null;
	barcode: string | null;
	source: string;
};
export type Portion = { phrase: string; food_id: number; grams: number };

/** A packaged product; values per 100 g. */
export type Product = Macros & {
	food_id: number;
	source: 'off' | 'user';
	barcode: string | null;
	name: string;
	brand: string | null;
	serving_g: number | null;
	serving_text: string | null;
	/** Set for Open Food Facts data: show it on the product sheet. */
	attribution: { text: string; url: string } | null;
};
export type SavedProduct = Product & { last_servings: number | null; last_used_at: number };
/** POST /api/products ("Enter from the label"). */
export type LabelIn = Macros & {
	name: string;
	brand?: string | null;
	barcode: string;
	basis: '100g' | 'serving';
	serving_g?: number | null;
	serving_text?: string | null;
};

export type ProviderKind = 'openai_compatible' | 'anthropic' | 'gemini';
export type Provider = {
	id: number;
	label: string;
	kind: ProviderKind;
	base_url: string;
	model: string;
	has_key: boolean;
	has_extra_headers: boolean;
	supports_vision: boolean;
	structured_output_mode: 'native' | 'prompt';
	is_local: boolean;
	is_default_text: boolean;
	is_default_vision: boolean;
};
/** Create/update body. On update, omit `api_key`/`extra_headers` to keep them, send null to clear. */
export type ProviderIn = Omit<Provider, 'id' | 'has_key' | 'has_extra_headers'> & {
	api_key?: string | null;
	extra_headers?: Record<string, string> | null;
};
export type ProviderTest = { ok: boolean; code?: string; message: string };

/** One entry of GET /api/trends; null means unknown (no meals, no profile yet, no weigh-in). */
export type TrendDay = {
	date: string;
	eaten_kcal: number | null;
	budget_kcal: number | null;
	weight_kg: number | null;
	steps: number | null;
	gym_minutes: number | null;
};

export type IngestToken = { id: number; label: string; created_at: string; last_used_at: string | null };

// ---- endpoints ----

const enc = encodeURIComponent;

export const health = () => api.get<Health>('/api/health');

export const auth = {
	state: () => api.get<AuthState>('/api/auth/state'),
	setup: (b: { username: string; password: string; timezone: string }) =>
		api.post<{ id: number }>('/api/auth/setup', b),
	login: (b: { username: string; password: string }) => api.post<{ id: number }>('/api/auth/login', b),
	logout: () => api.post<object>('/api/auth/logout')
};

export const profile = {
	get: () => api.get<Profile>('/api/profile'),
	save: (p: ProfileIn) => api.put<object>('/api/profile', p)
};

export const getBudget = (date: string) => api.get<Budget>(`/api/budget/${enc(date)}`);
export const getDay = (date: string) => api.get<Day>(`/api/days/${enc(date)}`);
export const getDays = (from: string, to: string) =>
	api.get<{ days: Day[] }>(`/api/days?from=${enc(from)}&to=${enc(to)}`).then((r) => r.days);
export const getTrends = (from: string, to: string) =>
	api.get<{ days: TrendDay[] }>(`/api/trends?from=${enc(from)}&to=${enc(to)}`).then((r) => r.days);

export const ingestTokens = {
	list: () => api.get<{ tokens: IngestToken[] }>('/api/ingest-tokens').then((r) => r.tokens),
	/** The plain token is in the reply once; the server keeps only a hash. */
	create: (label: string) =>
		api.post<IngestToken & { token: string }>('/api/ingest-tokens', { label }),
	remove: (id: number) => api.delete<object>(`/api/ingest-tokens/${id}`)
};

export const getMeals =(date: string) =>
	api.get<{ meals: Meal[] }>(`/api/meals?date=${enc(date)}`).then((r) => r.meals);

/** POST /api/estimate. `photo` should already be re-encoded through a canvas (strips EXIF). */
export function estimate(description: string, photo?: Blob): Promise<Estimate> {
	const form = new FormData();
	form.set('description', description);
	if (photo) form.set('photo', photo, 'photo');
	return api.postForm<Estimate>('/api/estimate', form);
}

export const foods = {
	create: (f: NewFood) => api.post<Food>('/api/foods', f),
	alias: (alias: string, food_id: number) => api.post<Alias>('/api/aliases', { alias, food_id }),
	aliases: () => api.get<{ aliases: AliasRow[] }>('/api/aliases').then((r) => r.aliases),
	removeAlias: (id: number) => api.delete<object>(`/api/aliases/${id}`),
	portion: (phrase: string, food_id: number, grams: number) =>
		api.post<Portion>('/api/portions', { phrase, food_id, grams })
};

export const products = {
	/** By barcode. Errors: 400 bad barcode, product_not_found, lookup_failed, rate_limited. */
	lookup: (barcode: string) => api.get<Product>(`/api/products/${enc(barcode)}`),
	label: (b: LabelIn) => api.post<Product>('/api/products', b),
	list: () => api.get<{ products: SavedProduct[] }>('/api/products').then((r) => r.products),
	/** By food id. */
	remove: (foodId: number) => api.delete<object>(`/api/products/${foodId}`)
};

export const providers = {
	list: () => api.get<{ providers: Provider[] }>('/api/ai/providers').then((r) => r.providers),
	create: (p: ProviderIn) => api.post<{ id: number }>('/api/ai/providers', p),
	update: (id: number, p: ProviderIn) => api.put<{ id: number }>(`/api/ai/providers/${id}`, p),
	remove: (id: number) => api.delete<object>(`/api/ai/providers/${id}`),
	/** Always 200 for an existing provider; read `ok`, `code` and `message`. */
	test: (id: number) => api.post<ProviderTest>(`/api/ai/providers/${id}/test`)
};

// ---- error messages (PLAN.md §9) ----

const MESSAGES: Record<string, string> = {
	no_provider:
		'No AI provider is set up yet. An admin can add one in Settings. You can still enter the meal by hand.',
	timeout: 'The AI provider took too long to answer. Try again, or enter the meal by hand.',
	unreachable:
		"Couldn't reach the AI provider. Check that it's running and reachable, then try again.",
	unparseable:
		"The AI provider's answer couldn't be read, even after a retry. Try again or reword the description.",
	daily_cap: "You've used today's AI estimates. Enter meals by hand until tomorrow.",
	provider_auth: 'The AI provider rejected its API key. An admin can update it in Settings.',
	product_not_found: "This product isn't in the database yet. Enter it from the label.",
	lookup_failed: "The product lookup didn't work. Try again, or enter it from the label."
};

/**
 * Plain-language text for an error from this module. `context` words network failures and the
 * shared `rate_limited` code (AI estimates vs product lookups). Screens decide where the message
 * goes; `no_provider` and `provider_auth` deserve a link to Settings.
 */
export function errorMessage(
	e: unknown,
	context: 'general' | 'estimate' | 'product' = 'general'
): string {
	if (e instanceof ApiError) {
		if (e.code === 'rate_limited' && context !== 'general') {
			return context === 'product'
				? 'Too many product lookups this hour. Enter it from the label instead.'
				: 'The AI provider is limiting requests right now. Wait a minute and try again.';
		}
		if (e.code === 'provider_error') return `The AI provider returned an error: ${e.message}`;
		return (e.code && MESSAGES[e.code]) || e.message;
	}
	if (e instanceof TypeError) {
		return {
			general: "Can't reach Cupful. Check your connection, then try again.",
			estimate: "Can't reach Cupful. Estimates need a connection; check it, then try again.",
			product: "Can't reach Cupful. Looking up a new product needs a connection."
		}[context];
	}
	return e instanceof Error ? e.message : String(e);
}
