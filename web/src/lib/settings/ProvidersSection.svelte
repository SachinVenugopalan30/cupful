<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  ProvidersSection — AI providers: list, add, edit, delete, defaults for text and photos,
  Test connection. Admin only (the routes answer 403 otherwise). Keys are write-only (`has_key`).
-->
<script lang="ts">
	import { ApiError, errorMessage, providers, type Provider } from '#lib/api.ts';
	import { connection } from '#lib/offline/state.svelte.ts';
	import { emptyForm, formFrom, KINDS, setKind, toProviderIn, type ProviderForm } from './providers.ts';

	let { isAdmin }: { isAdmin: boolean } = $props();

	let list = $state<Provider[] | null>(null);
	let listError = $state('');
	/** The open form: `id` null adds a provider. */
	let editing = $state<{ id: number | null; current: Provider | null; form: ProviderForm } | null>(null);
	let formError = $state('');
	let busy = $state(false);
	let status = $state('');
	let tests = $state<Record<number, { ok: boolean; message: string } | 'testing'>>({});

	async function load() {
		listError = '';
		try {
			list = await providers.list();
		} catch (e) {
			listError = errorMessage(e);
		}
	}
	$effect(() => {
		if (isAdmin) load();
	});

	function open(p: Provider | null) {
		formError = status = '';
		editing = { id: p?.id ?? null, current: p, form: p ? formFrom(p) : emptyForm() };
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!editing) return;
		formError = '';
		const r = toProviderIn(editing.form);
		if (!r.value) return void (formError = r.error!);
		busy = true;
		try {
			if (editing.id === null) await providers.create(r.value);
			else await providers.update(editing.id, r.value);
			status = `Saved ${r.value.label}.`;
			editing = null;
			await load();
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function remove(p: Provider) {
		if (!confirm(`Delete the provider “${p.label}”? Its saved key is deleted too.`)) return;
		status = '';
		try {
			await providers.remove(p.id);
			status = `Deleted ${p.label}.`;
			await load();
		} catch (err) {
			listError = errorMessage(err);
		}
	}

	async function test(p: Provider) {
		tests[p.id] = 'testing';
		try {
			const t = await providers.test(p.id);
			tests[p.id] = { ok: t.ok, message: t.ok ? t.message : errorMessage(new ApiError(200, t.message, t.code)) };
		} catch (err) {
			tests[p.id] = { ok: false, message: errorMessage(err) };
		}
	}
</script>

<section class="card stack" aria-labelledby="providers-h">
	<h2 id="providers-h">AI providers</h2>
	{#if !isAdmin}
		<p class="muted">Only the admin can change AI providers.</p>
	{:else}
		<p class="small muted">
			Meal descriptions, and photos when you use them, are sent to the provider set as default. Keys
			are encrypted on this server and never shown again. For Ollama or oMLX with Docker, see
			<code>docs/providers.md</code> in the source code.
		</p>
		{#if !connection.online}
			<p class="notice notice-warning">Settings need a connection. Provider changes are disabled offline.</p>
		{/if}
		{#if listError}<p class="notice notice-error" role="alert">{listError}</p>{/if}
		<p class="status small" role="status">{status}</p>

		{#if list === null && !listError}
			<div class="skeleton row-skel" aria-busy="true"></div>
		{:else if list?.length === 0}
			<p class="empty">No providers yet. Add one to estimate meals; manual entry works without one.</p>
		{:else if list}
			<ul class="list">
				{#each list as p (p.id)}
					{@const t = tests[p.id]}
					<li class="provider stack">
						<div class="stack head">
							<strong>{p.label}</strong>
							<span class="small muted wrap">{KINDS[p.kind].label} · {p.model}</span>
							<span class="small muted wrap">{p.base_url}</span>
							<span class="row">
								<span class="chip">{p.is_local ? 'Local' : 'Hosted'}</span>
								{#if p.is_default_text}<span class="chip">Default for text</span>{/if}
								{#if p.is_default_vision}<span class="chip">Default for photos</span>{/if}
								{#if p.supports_vision}<span class="chip">Reads photos</span>{/if}
								<span class="chip">{p.has_key ? 'Key saved' : 'No key'}</span>
							</span>
						</div>
						<div class="row">
							<button class="btn" onclick={() => test(p)} disabled={t === 'testing' || !connection.online}>
								{t === 'testing' ? 'Testing…' : 'Test connection'}
							</button>
							<button class="btn btn-quiet" onclick={() => open(p)} aria-label="Edit {p.label}">Edit</button>
							<button class="btn btn-quiet danger" onclick={() => remove(p)} disabled={!connection.online} aria-label="Delete {p.label}">Delete</button>
						</div>
						<p class="small" role="status">
							{#if t && t !== 'testing'}
								<span class={t.ok ? 'ok' : 'fail'}>{t.ok ? 'Connected.' : 'Connection failed.'}</span>
								{t.ok ? '' : t.message}
							{/if}
						</p>
					</li>
				{/each}
			</ul>
		{/if}

		{#if editing}
			{@const f = editing.form}
			<form class="stack edit" onsubmit={save} aria-labelledby="provider-form-h">
				<h3 id="provider-form-h">{editing.id === null ? 'Add a provider' : `Edit ${editing.current?.label}`}</h3>
				<div class="field">
					<label for="pv-kind">Kind</label>
					<select id="pv-kind" value={f.kind} onchange={(e) => (editing!.form = setKind(f, e.currentTarget.value as ProviderForm['kind']))}>
						{#each Object.entries(KINDS) as [k, v] (k)}<option value={k}>{v.label}</option>{/each}
					</select>
				</div>
				<div class="field">
					<label for="pv-label">Name</label>
					<input id="pv-label" bind:value={editing.form.label} maxlength="100" required />
				</div>
				<div class="field">
					<label for="pv-url">Base URL</label>
					<input id="pv-url" type="url" bind:value={editing.form.base_url} required autocapitalize="none" spellcheck="false" />
				</div>
				<div class="field">
					<label for="pv-model">Model</label>
					<input id="pv-model" bind:value={editing.form.model} required autocapitalize="none" spellcheck="false" />
				</div>
				<div class="field">
					<label for="pv-key">API key</label>
					<input id="pv-key" type="password" autocomplete="off" bind:value={editing.form.api_key} disabled={f.remove_key}
						placeholder={editing.current?.has_key ? 'Saved — leave blank to keep' : 'Optional for local models'}
						aria-describedby="pv-key-hint" />
					<p id="pv-key-hint" class="hint">
						{editing.current?.has_key ? 'Saved — leave blank to keep.' : 'Leave blank if the provider needs no key.'} Encrypted, never shown again.
					</p>
					{#if editing.current?.has_key}
						<label class="check"><input type="checkbox" bind:checked={editing.form.remove_key} />Remove key</label>
					{/if}
				</div>
				<div class="field">
					<label for="pv-headers">Extra headers (JSON, optional)</label>
					<textarea id="pv-headers" bind:value={editing.form.extra_headers} disabled={f.remove_headers} spellcheck="false"
						placeholder={editing.current?.has_extra_headers ? 'Saved — leave blank to keep' : '{"api-key": "…"}'}
						aria-describedby="pv-headers-hint"></textarea>
					<p id="pv-headers-hint" class="hint">Encrypted like the key and never shown again.</p>
					{#if editing.current?.has_extra_headers}
						<label class="check"><input type="checkbox" bind:checked={editing.form.remove_headers} />Remove extra headers</label>
					{/if}
				</div>
				<fieldset class="field">
					<legend class="label">Structured output</legend>
					<div class="segmented">
						<label><input type="radio" name="pv-mode" value="native" bind:group={editing.form.structured_output_mode} />Native</label>
						<label><input type="radio" name="pv-mode" value="prompt" bind:group={editing.form.structured_output_mode} />Prompt</label>
					</div>
					<p class="hint">Native uses the provider's JSON schema support. Switch to Prompt if the server rejects it.</p>
				</fieldset>
				<fieldset class="stack checks">
					<legend class="label">Options</legend>
					<label class="check"><input type="checkbox" bind:checked={editing.form.supports_vision}
						onchange={() => !f.supports_vision && (editing!.form.is_default_vision = false)} />The model can read photos</label>
					<label class="check"><input type="checkbox" bind:checked={editing.form.is_local} />Runs on my own hardware (no daily cap)</label>
					<label class="check"><input type="checkbox" bind:checked={editing.form.is_default_text} />Default for typed meals</label>
					<label class="check"><input type="checkbox" bind:checked={editing.form.is_default_vision} disabled={!f.supports_vision} />Default for photos</label>
				</fieldset>
				{#if formError}<p class="notice notice-error" role="alert">{formError}</p>{/if}
				<div class="row">
					<button class="btn btn-primary" disabled={busy || !connection.online}>{busy ? 'Saving…' : 'Save provider'}</button>
					<button type="button" class="btn btn-quiet" onclick={() => (editing = null)}>Cancel</button>
				</div>
			</form>
		{:else}
			<p><button class="btn" onclick={() => open(null)} disabled={!connection.online}>Add provider</button></p>
		{/if}
	{/if}
</section>

<style>
	.provider {
		align-items: stretch;
		gap: var(--space-2);
	}
	.head {
		gap: var(--space-1);
	}
	.wrap {
		overflow-wrap: anywhere;
	}
	.danger {
		color: var(--error);
	}
	.danger:hover {
		background: var(--error-soft);
		color: var(--error);
	}
	.ok {
		color: var(--under);
		font-weight: var(--weight-medium);
	}
	.fail {
		color: var(--error);
		font-weight: var(--weight-medium);
	}
	.status {
		color: var(--under);
	}
	.edit {
		border-top: 1px solid var(--border);
		padding-top: var(--space-4);
	}
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
		min-width: 0;
	}
	.checks {
		gap: var(--space-1);
	}
	.check {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--touch);
	}
	.row-skel {
		height: 5rem;
	}
	code {
		overflow-wrap: anywhere;
	}
</style>
