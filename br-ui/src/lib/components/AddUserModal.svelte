<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import type { UserRole } from '$lib/api/types';
	import Button from './ui/Button.svelte';
	import Input from './ui/Input.svelte';
	import { Shield, Eye, Check, X } from 'lucide-svelte';

	interface Props {
		onclose: () => void;
		oncreate: (data: { username: string; password: string; role: UserRole }) => void;
	}

	let { onclose, oncreate }: Props = $props();

	let username = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let role = $state<UserRole>('viewer');
	let showPassword = $state(false);

	// Password strength calculation
	const passwordStrength = $derived(() => {
		if (!password) return { score: 0, label: '', color: '' };

		let score = 0;
		if (password.length >= 8) score++;
		if (password.length >= 12) score++;
		if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
		if (/\d/.test(password)) score++;
		if (/[^a-zA-Z0-9]/.test(password)) score++;

		if (score <= 1) return { score, label: 'Weak', color: 'bg-red-500' };
		if (score <= 2) return { score, label: 'Fair', color: 'bg-orange-500' };
		if (score <= 3) return { score, label: 'Good', color: 'bg-amber-500' };
		if (score <= 4) return { score, label: 'Strong', color: 'bg-emerald-500' };
		return { score, label: 'Very Strong', color: 'bg-emerald-400' };
	});

	const passwordsMatch = $derived(password === confirmPassword && password.length > 0);
	const canCreate = $derived(username.trim().length > 0 && password.length >= 8 && passwordsMatch);

	function handleCreate() {
		if (!canCreate) return;
		oncreate({
			username: username.trim(),
			password,
			role
		});
	}

	function handleOpenChange(open: boolean) {
		if (!open) onclose();
	}
</script>

<ResponsiveModal open={true} onOpenChange={handleOpenChange} title="Add User">
	{#snippet children()}
		<form
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleCreate();
			}}
		>
			<!-- Username -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Username
				</span>
				<Input type="text" placeholder="username" bind:value={username} autocomplete="off" />
			</div>

			<!-- Password -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Password
				</span>
				<div class="relative">
					<Input
						type={showPassword ? 'text' : 'password'}
						class="pr-10"
						placeholder="min 8 characters"
						bind:value={password}
						autocomplete="new-password"
					/>
					<button
						type="button"
						class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
						onclick={() => (showPassword = !showPassword)}
					>
						{#if showPassword}
							<Eye size={16} />
						{:else}
							<Eye size={16} class="opacity-50" />
						{/if}
					</button>
				</div>

				<!-- Password Strength Indicator -->
				{#if password.length > 0}
					<div class="mt-2">
						<div class="flex gap-1 mb-1">
							{#each Array(5) as _, i}
								<div
									class="h-1 flex-1 rounded-full transition-colors {i < passwordStrength().score
										? passwordStrength().color
										: 'bg-muted'}"
								></div>
							{/each}
						</div>
						<span class="font-mono text-[10px] text-muted-foreground">{passwordStrength().label}</span>
					</div>
				{/if}
			</div>

			<!-- Confirm Password -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Confirm Password
				</span>
				<div class="relative">
					<Input
						type={showPassword ? 'text' : 'password'}
						class="pr-10"
						placeholder="confirm password"
						bind:value={confirmPassword}
						autocomplete="new-password"
					/>
					{#if confirmPassword.length > 0}
						<span class="absolute right-2 top-1/2 -translate-y-1/2">
							{#if passwordsMatch}
								<Check size={16} class="text-emerald-400" />
							{:else}
								<X size={16} class="text-red-400" />
							{/if}
						</span>
					{/if}
				</div>
				{#if confirmPassword.length > 0 && !passwordsMatch}
					<span class="font-mono text-[10px] text-red-400 mt-1 block">Passwords do not match</span>
				{/if}
			</div>

			<!-- Role Selector -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Role
				</span>
				<div class="flex rounded border border-border overflow-hidden">
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors {role ===
						'admin'
							? 'bg-muted text-foreground'
							: 'bg-input text-muted-foreground hover:bg-muted/50'}"
						onclick={() => (role = 'admin')}
					>
						<Shield size={14} />
						Admin
					</button>
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors {role ===
						'viewer'
							? 'bg-muted text-foreground'
							: 'bg-input text-muted-foreground hover:bg-muted/50'}"
						onclick={() => (role = 'viewer')}
					>
						<Eye size={14} />
						Viewer
					</button>
				</div>
				<p class="font-mono text-[10px] text-muted-foreground mt-2">
					{#if role === 'admin'}
						Full access: manage channels, recordings, users, and settings
					{:else}
						Read-only access: view channels, recordings, and status
					{/if}
				</p>
			</div>
		</form>
	{/snippet}

	{#snippet footer()}
		<div class="flex gap-2">
			<Button type="button" intent="primary" fullWidth disabled={!canCreate} onclick={handleCreate}>
				Create User
			</Button>
			<Button type="button" onclick={onclose}>Cancel</Button>
		</div>
	{/snippet}
</ResponsiveModal>
