// SPDX-License-Identifier: AGPL-3.0-or-later
// Types matching the server's JSON, grouped by the resource that returns them.

import type { WeightUnit, HeightUnit } from '../format';

// ---- account and health ----

export type Health = { status: string; version: string; source_url: string };

export type AuthUser = { id: number; username: string; is_admin: boolean };
/** `tailscale` when the server signs people in from the Tailscale header instead of a password. */
export type IdentityMode = 'tailscale' | null;
export type AuthState = {
	needs_setup: boolean;
	user: AuthUser | null;
	identity_mode: IdentityMode;
};

/** GET /api/invites item: a pending invite. Times are RFC 3339. */
export type Invite = { id: number; created_at: string; expires_at: string };
/** POST /api/invites answer: `token` is returned once and never listed again. */
export type NewInvite = Invite & { token: string };

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

// ---- days, budget and trends ----

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

/** One entry of GET /api/trends; null means unknown (no meals, no profile yet, no weigh-in). */
export type TrendDay = {
	date: string;
	eaten_kcal: number | null;
	budget_kcal: number | null;
	weight_kg: number | null;
	steps: number | null;
	gym_minutes: number | null;
};

// ---- meals and estimates ----

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

// ---- foods and products ----

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

// ---- AI providers ----

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

// ---- activity ingest and Google ----

export type IngestToken = { id: number; label: string; created_at: string; last_used_at: string | null };

export type GoogleStatus =
	| { configured: false }
	| { configured: true; connected: false }
	| {
			configured: true;
			connected: true;
			needs_reconnect: boolean;
			last_sync_at: string | null;
			last_error: string | null;
	  };

// ---- import, backup and restore ----

/**
 * POST /api/import/fuel-ledger answer. `meals_unchanged` were already identical; the `skipped`
 * ones were left alone because they were deleted, or edited in Cupful, since.
 */
export type ImportSummary = {
	meals_created: number;
	meals_updated: number;
	meals_unchanged: number;
	meals_skipped_deleted: number;
	meals_skipped_edited: number;
	items: number;
	days: number;
};

/** POST /api/restore answer: how many rows of each kind the backup replaced. */
export type RestoreSummary = {
	profile_versions: number;
	days: number;
	meals: number;
	items: number;
	foods: number;
	aliases: number;
	portions: number;
	products: number;
};
