<script lang="ts">
	import { Users, Plus, Edit, Trash2, Key, Shield, Eye } from 'lucide-svelte';
	import { usersStore } from '$lib/stores/users.svelte';
	import { connectionStore, breakpointStore, toastStore } from '$lib';
	import AddUserModal from '$lib/components/AddUserModal.svelte';
	import EditUserModal from '$lib/components/EditUserModal.svelte';
	import UserSessionsPanel from '$lib/components/UserSessionsPanel.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import { untrack } from 'svelte';
	import type { User, CreateUserRequest, UpdateUserRequest } from '$lib/api/types';

	let showAddModal = $state(false);
	let editingUser = $state<User | null>(null);
	let sessionsUser = $state<User | null>(null);
	let deleteConfirmUser = $state<User | null>(null);

	// Reload users when server changes or connection is established
	$effect(() => {
		// Track activeServerId to detect server switches
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				usersStore.setCurrentUsername(connectionStore.username);
				usersStore.load(serverId);
			});
		}
	});

	async function handleCreateUser(data: CreateUserRequest) {
		const success = await usersStore.createUser(data);
		if (success) {
			showAddModal = false;
			toastStore.success(`User "${data.username}" created`);
		} else {
			toastStore.error(usersStore.error ?? 'Failed to create user');
		}
	}

	async function handleSaveUser(data: UpdateUserRequest) {
		if (!editingUser) return;
		const success = await usersStore.updateUser(editingUser.id, data);
		if (success) {
			editingUser = null;
			toastStore.success('User updated');
		} else {
			toastStore.error(usersStore.error ?? 'Failed to update user');
		}
	}

	async function handleDeleteUser() {
		if (!deleteConfirmUser) return;
		const username = deleteConfirmUser.username;
		const success = await usersStore.deleteUser(deleteConfirmUser.id);
		if (success) {
			deleteConfirmUser = null;
			toastStore.success(`User "${username}" deleted`);
		} else {
			toastStore.error(usersStore.error ?? 'Failed to delete user');
		}
	}

	function formatLastLogin(dateStr?: string): string {
		if (!dateStr) return 'Never';

		const date = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / (1000 * 60));
		const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

		if (diffMins < 1) return 'Just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		if (diffHours < 24) {
			const hours = date.getHours().toString().padStart(2, '0');
			const mins = date.getMinutes().toString().padStart(2, '0');
			return `Today ${hours}:${mins}`;
		}
		if (diffDays === 1) {
			const hours = date.getHours().toString().padStart(2, '0');
			const mins = date.getMinutes().toString().padStart(2, '0');
			return `Yesterday ${hours}:${mins}`;
		}
		return `${diffDays}d ago`;
	}
</script>

<div class="space-y-4">
	<!-- Header Bar -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex items-center gap-3">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Users</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{usersStore.userCount}
			</span>
			{#if usersStore.onlineCount > 0}
				<span
					class="rounded bg-emerald-500/20 px-1.5 py-0.5 font-mono text-[10px] text-emerald-400"
				>
					{usersStore.onlineCount} online
				</span>
			{/if}
		</div>

		<button
			class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs flex items-center justify-center gap-2 hover:bg-muted transition-colors w-full sm:w-auto"
			onclick={() => (showAddModal = true)}
		>
			<Plus class="w-3.5 h-3.5" />
			Add User
		</button>
	</div>

	<!-- Users Table/Cards -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">Not connected to a server.</p>
		</div>
	{:else if usersStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading users...</span>
		</div>
	{:else if usersStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{usersStore.error}</p>
		</div>
	{:else if usersStore.users.length === 0}
		<div class="relative border border-border bg-card p-8">
			<CornerBrackets />

			<div class="flex flex-col items-center justify-center gap-2 text-zinc-500">
				<Users class="size-8 opacity-30" />
				<p class="font-mono text-xs">No users found</p>
				<button
					class="mt-2 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
					onclick={() => (showAddModal = true)}
				>
					Add your first user
				</button>
			</div>
		</div>
	{:else if breakpointStore.isMobile}
		<!-- Mobile: Card Layout -->
		<div class="space-y-2">
			{#each usersStore.users as user (user.id)}
				{@const isCurrentUser = usersStore.isCurrentUser(user)}
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets size="sm" />

					<div class="flex items-start justify-between gap-3">
						<div class="flex items-center gap-3">
							<!-- Online indicator -->
							<span
								class="size-2.5 rounded-full flex-shrink-0 {user.is_online
									? 'bg-emerald-400'
									: 'bg-zinc-500'}"
							></span>
							<div>
								<div class="flex items-center gap-2">
									<span class="font-mono text-sm text-zinc-100">{user.username}</span>
									{#if isCurrentUser}
										<span
											class="rounded bg-blue-500/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue-400"
										>
											You
										</span>
									{/if}
								</div>
								<div class="flex items-center gap-2 mt-1">
									<span
										class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
									>
										{user.role}
									</span>
									<span class="font-mono text-[10px] text-zinc-500">
										{formatLastLogin(user.last_login)}
									</span>
								</div>
							</div>
						</div>

						<!-- Actions -->
						{#if !isCurrentUser}
							<div class="flex items-center gap-1">
								<button
									class="p-1.5 hover:bg-muted rounded transition-colors"
									title="Edit"
									onclick={() => (editingUser = user)}
								>
									<Edit class="w-3.5 h-3.5 text-zinc-500" />
								</button>
								<button
									class="p-1.5 hover:bg-muted rounded transition-colors"
									title="Sessions"
									onclick={() => (sessionsUser = user)}
								>
									<Key class="w-3.5 h-3.5 text-zinc-500" />
								</button>
								<button
									class="p-1.5 hover:bg-red-500/10 rounded transition-colors"
									title="Delete"
									onclick={() => (deleteConfirmUser = user)}
								>
									<Trash2 class="w-3.5 h-3.5 text-red-400" />
								</button>
							</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<!-- Desktop/Tablet: Table Layout -->
		<div class="relative border border-border bg-card overflow-hidden">
			<CornerBrackets class="z-10" />

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-16"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500"
							>Username</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-24"
							>Role</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-32"
							>Last Login</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-32"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each usersStore.users as user (user.id)}
						{@const isCurrentUser = usersStore.isCurrentUser(user)}
						<tr class="border-b border-border/30 hover:bg-muted/30 transition-colors">
							<!-- Status -->
							<td class="px-4 py-3">
								<div class="flex items-center gap-1.5">
									<span
										class="size-2 rounded-full {user.is_online ? 'bg-emerald-400' : 'bg-zinc-500'}"
									></span>
									<span class="sr-only">{user.is_online ? 'Online' : 'Offline'}</span>
								</div>
							</td>

							<!-- Username -->
							<td class="px-4 py-3">
								<div class="flex items-center gap-2">
									<span class="font-mono text-sm">{user.username}</span>
									{#if isCurrentUser}
										<span
											class="rounded bg-blue-500/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue-400"
										>
											You
										</span>
									{/if}
								</div>
							</td>

							<!-- Role -->
							<td class="px-4 py-3">
								<div class="flex items-center gap-1.5">
									{#if user.role === 'admin'}
										<Shield class="w-3.5 h-3.5 text-amber-400" />
									{:else}
										<Eye class="w-3.5 h-3.5 text-zinc-500" />
									{/if}
									<span
										class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
									>
										{user.role}
									</span>
								</div>
							</td>

							<!-- Last Login -->
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-zinc-400">
									{formatLastLogin(user.last_login)}
								</span>
							</td>

							<!-- Actions -->
							<td class="px-4 py-3">
								{#if isCurrentUser}
									<span class="font-mono text-[10px] text-zinc-500">-</span>
								{:else}
									<div class="flex items-center gap-1">
										<button
											class="p-1.5 hover:bg-muted rounded transition-colors"
											title="Edit"
											onclick={() => (editingUser = user)}
										>
											<Edit class="w-3.5 h-3.5 text-zinc-500" />
										</button>
										<button
											class="p-1.5 hover:bg-muted rounded transition-colors"
											title="Sessions"
											onclick={() => (sessionsUser = user)}
										>
											<Key class="w-3.5 h-3.5 text-zinc-500" />
										</button>
										<button
											class="p-1.5 hover:bg-red-500/10 rounded transition-colors"
											title="Delete"
											onclick={() => (deleteConfirmUser = user)}
										>
											<Trash2 class="w-3.5 h-3.5 text-red-400" />
										</button>
									</div>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<!-- Add User Modal -->
{#if showAddModal}
	<AddUserModal onclose={() => (showAddModal = false)} oncreate={handleCreateUser} />
{/if}

<!-- Edit User Modal -->
{#if editingUser}
	<EditUserModal
		user={editingUser}
		isCurrentUser={usersStore.isCurrentUser(editingUser)}
		onclose={() => (editingUser = null)}
		onsave={handleSaveUser}
	/>
{/if}

<!-- User Sessions Panel -->
{#if sessionsUser}
	<UserSessionsPanel user={sessionsUser} onclose={() => (sessionsUser = null)} />
{/if}

<!-- Delete Confirmation Modal -->
{#if deleteConfirmUser}
	{@const user = deleteConfirmUser}
	<div class="fixed inset-0 bg-black/60 z-40 flex items-center justify-center p-4">
		<div class="relative border border-zinc-700 bg-zinc-900 w-full max-w-sm p-6">
			<CornerBrackets size="lg" />

			<h3 class="font-mono text-xs uppercase tracking-wider text-zinc-400 mb-4">Delete User</h3>
			<p class="font-mono text-sm text-zinc-300 mb-2">
				Are you sure you want to delete "{user.username}"?
			</p>
			{#if user.is_online}
				<p class="font-mono text-[10px] text-amber-400 mb-4">
					This user has active sessions that will be revoked.
				</p>
			{/if}
			<p class="font-mono text-[10px] text-zinc-500 mb-6">This action cannot be undone.</p>

			<div class="flex gap-2">
				<button
					class="flex-1 rounded bg-red-600 px-4 py-2 font-mono text-sm text-white hover:bg-red-500 transition-colors"
					onclick={handleDeleteUser}
				>
					Delete
				</button>
				<button
					class="rounded border border-zinc-700 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
					onclick={() => (deleteConfirmUser = null)}
				>
					Cancel
				</button>
			</div>
		</div>
	</div>
{/if}
