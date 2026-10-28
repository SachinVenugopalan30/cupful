// SPDX-License-Identifier: AGPL-3.0-or-later
// The AI provider form. Keys and extra headers are write-only: blank keeps what's stored,
// "Remove" sends null (server/src/ai/settings.rs).

import type { Provider, ProviderIn, ProviderKind } from '../api';

export const KINDS: Record<ProviderKind, { label: string; base_url: string; is_local: boolean }> = {
	openai_compatible: { label: 'OpenAI-compatible (OpenAI, Ollama, …)', base_url: 'http://host.docker.internal:11434/v1', is_local: true },
	anthropic: { label: 'Anthropic', base_url: 'https://api.anthropic.com/v1', is_local: false },
	gemini: { label: 'Gemini', base_url: 'https://generativelanguage.googleapis.com/v1beta', is_local: false }
};

export type ProviderForm = Omit<ProviderIn, 'api_key' | 'extra_headers'> & {
	api_key: string;
	remove_key: boolean;
	/** JSON object text. */
	extra_headers: string;
	remove_headers: boolean;
};

export function emptyForm(): ProviderForm {
	return {
		label: '',
		kind: 'openai_compatible',
		base_url: KINDS.openai_compatible.base_url,
		model: '',
		api_key: '',
		remove_key: false,
		extra_headers: '',
		remove_headers: false,
		supports_vision: false,
		structured_output_mode: 'native',
		is_local: KINDS.openai_compatible.is_local,
		is_default_text: false,
		is_default_vision: false
	};
}

export function formFrom(p: Provider): ProviderForm {
	const { id, has_key, has_extra_headers, ...rest } = p;
	return { ...rest, api_key: '', remove_key: false, extra_headers: '', remove_headers: false };
}

/** Switches kind, replacing the base URL and "local" only while they are the old kind's defaults. */
export function setKind(f: ProviderForm, kind: ProviderKind): ProviderForm {
	const old = KINDS[f.kind];
	const isDefault = !f.base_url || f.base_url === old.base_url;
	return {
		...f,
		kind,
		base_url: isDefault ? KINDS[kind].base_url : f.base_url,
		is_local: isDefault ? KINDS[kind].is_local : f.is_local
	};
}

export function toProviderIn(f: ProviderForm): { value?: ProviderIn; error?: string } {
	const { api_key, remove_key, extra_headers, remove_headers, ...rest } = f;
	const value: ProviderIn = { ...rest, label: rest.label.trim(), model: rest.model.trim(), base_url: rest.base_url.trim() };
	if (remove_key) value.api_key = null;
	else if (api_key.trim()) value.api_key = api_key.trim();
	if (remove_headers) value.extra_headers = null;
	else if (extra_headers.trim()) {
		let h: unknown;
		try {
			h = JSON.parse(extra_headers);
		} catch {
			h = null;
		}
		if (!h || typeof h !== 'object' || Array.isArray(h) || Object.values(h).some((v) => typeof v !== 'string'))
			return { error: 'Extra headers must be a JSON object of text values, like {"api-key": "…"}.' };
		value.extra_headers = h as Record<string, string>;
	}
	return { value };
}
