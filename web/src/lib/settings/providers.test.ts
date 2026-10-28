// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Provider } from '../api';
import { emptyForm, formFrom, setKind, toProviderIn } from './providers';

const stored: Provider = {
	id: 3,
	label: 'Ollama',
	kind: 'openai_compatible',
	base_url: 'http://host.docker.internal:11434/v1',
	model: 'llama3.2',
	has_key: true,
	has_extra_headers: true,
	supports_vision: false,
	structured_output_mode: 'native',
	is_local: true,
	is_default_text: true,
	is_default_vision: false
};

describe('provider form', () => {
	it('fills sensible defaults per kind, keeping a URL the user typed', () => {
		expect(emptyForm()).toMatchObject({ kind: 'openai_compatible', base_url: 'http://host.docker.internal:11434/v1', is_local: true });
		const a = setKind(emptyForm(), 'anthropic');
		expect(a).toMatchObject({ base_url: 'https://api.anthropic.com/v1', is_local: false });
		expect(setKind(a, 'gemini').base_url).toBe('https://generativelanguage.googleapis.com/v1beta');
		expect(setKind({ ...a, base_url: 'https://proxy.example/v1' }, 'gemini').base_url).toBe('https://proxy.example/v1');
	});

	it('omits a blank key and blank headers, so an edit keeps them', () => {
		const p = toProviderIn(formFrom(stored));
		expect(p.error).toBeUndefined();
		expect(p.value).not.toHaveProperty('api_key');
		expect(p.value).not.toHaveProperty('extra_headers');
		expect(p.value).toMatchObject({ label: 'Ollama', model: 'llama3.2', is_default_text: true });
	});

	it('sends a typed key and parsed headers', () => {
		const f = { ...emptyForm(), label: 'x', model: 'm', api_key: ' sk-1 ', extra_headers: '{"api-key":"k"}' };
		expect(toProviderIn(f).value).toMatchObject({ api_key: 'sk-1', extra_headers: { 'api-key': 'k' } });
	});

	it('sends null when the key or headers are removed', () => {
		const f = { ...formFrom(stored), remove_key: true, remove_headers: true, api_key: 'ignored' };
		expect(toProviderIn(f).value).toMatchObject({ api_key: null, extra_headers: null });
	});

	it('rejects headers that are not a JSON object of strings', () => {
		for (const h of ['nope', '[1]', '{"a":1}']) expect(toProviderIn({ ...emptyForm(), extra_headers: h }).error).toMatch(/JSON/);
	});
});
