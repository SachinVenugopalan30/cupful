<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import LogoutButton from '#lib/LogoutButton.svelte';
	import CountUp from '#lib/motion/CountUp.svelte';
	import ProfileSection from '#lib/settings/ProfileSection.svelte';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import { ApiError, errorMessage, getBudget } from '#lib/api.ts';
	import { session, today } from '#lib/session.svelte.ts';

	type Plan =
		| { state: 'loading' }
		| { state: 'ready'; kcal: number; protein: number }
		| { state: 'none' }
		| { state: 'error'; message: string };
	let plan = $state<Plan>({ state: 'loading' });

	/**
	 * Today's server budget. `getBudget` goes to the network first and writes the answer through to the
	 * offline cache, so a refetch after a save is fresh; the cached copy is only the offline fallback.
	 */
	async function loadPlan() {
		try {
			const b = await getBudget(today());
			plan = { state: 'ready', kcal: b.budget, protein: b.protein_g };
		} catch (e) {
			// 409: no profile yet, so there is no plan to show.
			plan = e instanceof ApiError && e.status === 409 ? { state: 'none' } : { state: 'error', message: errorMessage(e) };
		}
	}
	loadPlan();
</script>

<svelte:head><title>Profile · Cupful</title></svelte:head>

<div class="profile stack">
	<header class="page-head">
		<h1>Profile</h1>
		<a class="btn btn-quiet" href="/settings">Settings</a>
	</header>
	<p class="muted small">Targets, units and time zone. Saving creates a new profile version from today.</p>

	<section class="card plan" aria-labelledby="plan-h" aria-busy={plan.state === 'loading'}>
		<div class="card-head">
			<h2 id="plan-h">Your daily plan</h2>
			<span>from today</span>
		</div>
		{#if plan.state === 'ready'}
			<p class="fig"><CountUp value={plan.kcal} /><small> kcal a day</small></p>
			<p class="note">{plan.protein} g protein. The server works this out from your profile and activity.</p>
		{:else if plan.state === 'none'}
			<p class="note">Save your profile to see your daily plan.</p>
		{:else if plan.state === 'error'}
			<p class="note">{plan.message}</p>
		{:else}
			<div class="skeleton plan-skel"></div>
		{/if}
	</section>

	<ProfileSection onSaved={loadPlan} />

	<SettingRow headingId="account-h" flat title="Account" description="Log out of this device.">
		<p>Logged in as <strong>{session.user?.username}</strong>.</p>
		<LogoutButton />
	</SettingRow>
</div>

<style>
	.profile {
		max-width: var(--page-max);
		margin: 0 auto;
	}
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.plan {
		display: grid;
		gap: var(--space-2);
	}
	.plan .note {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}
	.plan-skel {
		block-size: 2.5rem;
		max-inline-size: 12rem;
	}
	@media (prefers-reduced-motion: no-preference) {
		.plan {
			animation: rise var(--dur-med) var(--ease) both;
		}
	}
</style>
