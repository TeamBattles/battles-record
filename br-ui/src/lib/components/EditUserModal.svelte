<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import type { User, UserRole, UpdateUserRequest } from '$lib/api/types';
	import Button from './ui/Button.svelte';
	import Input from './ui/Input.svelte';
	import Checkbox from './ui/Checkbox.svelte';
	import { Shield, Eye, Check, X, AlertTriangle } from 'lucide-svelte';

	interface Props {
		user: User;
		isCurrentUser: boolean;
		onclose: () => void;
		onsave: (data: UpdateUserRequest) => void;
	}

	let { user, isCurrentUser, onclose, onsave }: Props = $props();

	let role = $state<UserRole>('viewer');
	let resetPassword = $state(false);

	// Sync role from user prop
	$effect(() => {
		role = user.role;
	});
	let newPassword = $state('');
	let confirmPassword = $state('');
	let showPassword = $state(false);

	// Password strength calculation
	const passwordStrength = $derived(() => {
		if (!newPassword) return { score: 0, label: '', color: '' };

		let score = 0;
		if (newPassword.length >= 8) score++;
		if (newPassword.length >= 12) score++;
		if (/[a-z]/.test(newPassword) && /[A-Z]/.test(newPassword)) score++;
		if (/\d/.test(newPassword)) score++;
		if (/[^a-zA-Z0-9]/.test(newPassword)) score++;

		if (score <= 1) return { score, label: 'Weak', color: 'bg-red-500' };
		if (score <= 2) return { score, label: 'Fair', color: 'bg-orange-500' };
		if (score <= 3) return { score, label: 'Good', color: 'bg-amber-500' };
		if (score <= 4) return { score, label: 'Strong', color: 'bg-emerald-500' };
		return { score, label: 'Very Strong', color: 'bg-emerald-400' };
	});

	const passwordsMatch = $derived(newPassword === confirmPassword && newPassword.length > 0);
	const hasRoleChange = $derived(role !== user.role);
	const hasPasswordChange = $derived(resetPassword && newPassword.length >= 8 && passwordsMatch);
	const canSave = $derived(hasRoleChange || hasPasswordChange);

	function handleSave() {
		if (!canSave) return;

		const data: UpdateUserRequest = {};
		if (hasRoleChange) {
			data.role = role;
		}
		if (hasPasswordChange) {
			data.password = newPassword;
		}

		onsave(data);
	}

	function handleOpenChange(open: boolean) {
		if (!open) onclose();
	}
</script>

<ResponsiveModal open={true} onOpenChange={handleOpenChange} title="Edit User">
	{#snippet children()}
		<form
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleSave();
			}}
		>
			<!-- Username (read-only) -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Username
				</span>
				<div class="flex items-center gap-2">
					<span class="font-mono text-sm text-zinc-300">{user.username}</span>
					{#if isCurrentUser}
						<span
							class="rounded bg-blue-500/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue-400"
						>
							You
						</span>
					{/if}
				</div>
			</div>

			<!-- Role Selector -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Role
				</span>
				<div class="flex rounded border border-zinc-700 overflow-hidden">
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors {role ===
						'admin'
							? 'bg-zinc-700 text-zinc-100'
							: 'bg-zinc-800 text-zinc-500 hover:bg-zinc-700/50'}"
						onclick={() => (role = 'admin')}
					>
						<Shield size={14} />
						Admin
					</button>
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors disabled:opacity-50 disabled:cursor-not-allowed {role ===
						'viewer'
							? 'bg-zinc-700 text-zinc-100'
							: 'bg-zinc-800 text-zinc-500 hover:bg-zinc-700/50'}"
						onclick={() => (role = 'viewer')}
						disabled={isCurrentUser && user.role === 'admin'}
					>
						<Eye size={14} />
						Viewer
					</button>
				</div>
				{#if isCurrentUser && user.role === 'admin'}
					<div class="flex items-center gap-1.5 mt-2">
						<AlertTriangle size={12} class="text-amber-400" />
						<span class="font-mono text-[10px] text-amber-400">
							You cannot demote yourself from admin
						</span>
					</div>
				{/if}
			</div>

			<!-- Reset Password Toggle -->
			<div class="flex items-center justify-between rounded border border-zinc-700 bg-zinc-800 p-3">
				<div>
					<p class="font-mono text-sm text-zinc-200">Reset Password</p>
					<p class="font-mono text-[10px] text-zinc-500">Set a new password for this user</p>
				</div>
				<Checkbox bind:checked={resetPassword} />
			</div>

			{#if resetPassword}
				<!-- New Password -->
				<div>
					<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
						New Password
					</span>
					<div class="relative">
						<Input
							type={showPassword ? 'text' : 'password'}
							class="pr-10"
							placeholder="min 8 characters"
							bind:value={newPassword}
							autocomplete="new-password"
						/>
						<button
							type="button"
							class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-zinc-500 hover:text-zinc-300"
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
					{#if newPassword.length > 0}
						<div class="mt-2">
							<div class="flex gap-1 mb-1">
								{#each Array(5) as _, i}
									<div
										class="h-1 flex-1 rounded-full transition-colors {i < passwordStrength().score
											? passwordStrength().color
											: 'bg-zinc-700'}"
									></div>
								{/each}
							</div>
							<span class="font-mono text-[10px] text-zinc-500">{passwordStrength().label}</span>
						</div>
					{/if}
				</div>

				<!-- Confirm Password -->
				<div>
					<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
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
						<span class="font-mono text-[10px] text-red-400 mt-1 block">Passwords do not match</span
						>
					{/if}
				</div>
			{/if}
		</form>
	{/snippet}

	{#snippet footer()}
		<div class="flex gap-2">
			<Button type="button" intent="primary" fullWidth disabled={!canSave} onclick={handleSave}>
				Save Changes
			</Button>
			<Button type="button" onclick={onclose}>Cancel</Button>
		</div>
	{/snippet}
</ResponsiveModal>
