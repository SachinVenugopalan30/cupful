// SPDX-License-Identifier: AGPL-3.0-or-later
// The API error type and the plain-language messages screens show (PLAN.md §9).

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
