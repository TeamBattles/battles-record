<script lang="ts">
	import { ResponsiveModal } from '#lib';
	import type { User, UserRole, UpdateUserRequest } from '#lib/api/types.ts';
	import Button from './ui/Button.svelte';
	import Input from './ui/Input.svelte';
	import Checkbox from './ui/Checkbox.svelte';
	import { Shield, Eye, Check, X, AlertTriangle } from '@lucide/svelte';

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

		if (score <= 1) return { score, label: 'Weak', color: 'bg-destructive' };
		if (score <= 2) return { score, label: 'Fair', color: 'bg-orange' };
		if (score <= 3) return { score, label: 'Good', color: 'bg-yellow' };
		if (score <= 4) return { score, label: 'Strong', color: 'bg-green' };
		return { score, label: 'Very Strong', color: 'bg-green' };
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
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Username
				</span>
				<div class="flex items-center gap-2">
					<span class="font-mono text-sm text-text-2">{user.username}</span>
					{#if isCurrentUser}
						<span
							class="rounded bg-blue/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue"
						>
							You
						</span>
					{/if}
				</div>
			</div>

			<!-- Role Selector -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Role
				</span>
				<div class="flex rounded border border-line-2 overflow-hidden">
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors {role ===
						'admin'
							? 'bg-chip-strong text-foreground'
							: 'bg-btn-bg text-muted-foreground hover:bg-hover-wash'}"
						onclick={() => (role = 'admin')}
					>
						<Shield size={14} />
						Admin
					</button>
					<button
						type="button"
						class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors disabled:opacity-50 disabled:cursor-not-allowed {role ===
						'viewer'
							? 'bg-chip-strong text-foreground'
							: 'bg-btn-bg text-muted-foreground hover:bg-hover-wash'}"
						onclick={() => (role = 'viewer')}
						disabled={isCurrentUser && user.role === 'admin'}
					>
						<Eye size={14} />
						Viewer
					</button>
				</div>
				{#if isCurrentUser && user.role === 'admin'}
					<div class="flex items-center gap-1.5 mt-2">
						<AlertTriangle size={12} class="text-yellow" />
						<span class="font-mono text-[10px] text-yellow">
							You cannot demote yourself from admin
						</span>
					</div>
				{/if}
			</div>

			<!-- Reset Password Toggle -->
			<div class="flex items-center justify-between rounded border border-line-2 bg-chip p-3">
				<div>
					<p class="font-mono text-sm text-foreground">Reset Password</p>
					<p class="font-mono text-[10px] text-muted-foreground">Set a new password for this user</p>
				</div>
				<Checkbox bind:checked={resetPassword} />
			</div>

			{#if resetPassword}
				<!-- New Password -->
				<div>
					<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
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
					{#if newPassword.length > 0}
						<div class="mt-2">
							<div class="flex gap-1 mb-1">
								{#each Array(5) as _, i}
									<div
										class="h-1 flex-1 rounded-full transition-colors {i < passwordStrength().score
											? passwordStrength().color
											: 'bg-chip-strong'}"
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
									<Check size={16} class="text-green" />
								{:else}
									<X size={16} class="text-destructive" />
								{/if}
							</span>
						{/if}
					</div>
					{#if confirmPassword.length > 0 && !passwordsMatch}
						<span class="font-mono text-[10px] text-destructive mt-1 block">Passwords do not match</span
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
