<script lang="ts">
	import PageHeader from '#lib/components/ui/PageHeader.svelte';
	import { Users, Plus, Edit, Trash2, Key, Shield, Eye } from '@lucide/svelte';
	import { usersStore } from '#lib/stores/users.svelte.ts';
	import { connectionStore, breakpointStore, toastStore } from '#lib';
	import AddUserModal from '#lib/components/AddUserModal.svelte';
	import EditUserModal from '#lib/components/EditUserModal.svelte';
	import UserSessionsPanel from '#lib/components/UserSessionsPanel.svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import { untrack } from 'svelte';
	import type { User, CreateUserRequest, UpdateUserRequest } from '#lib/api/types.ts';

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
	<PageHeader
		title="Users"
		eyebrow="Admin"
		subtitle="Accounts with access to this server"
		icon={Users}
		count={usersStore.userCount}
	>
		{#snippet badges()}
			{#if usersStore.onlineCount > 0}
				<span
					class="rounded bg-green/20 px-1.5 py-0.5 font-mono text-[10px] text-green"
				>
					{usersStore.onlineCount} online
				</span>
			{/if}
		{/snippet}
		{#snippet actions()}
			<button
				class="rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs flex items-center justify-center gap-2 hover:bg-hover-wash transition-colors w-full sm:w-auto"
				onclick={() => (showAddModal = true)}
			>
				<Plus class="w-3.5 h-3.5" />
				Add User
			</button>
		{/snippet}
	</PageHeader>

	<!-- Users Table/Cards -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-yellow/30 bg-yellow/5 p-4">
			<p class="font-mono text-xs text-yellow">Not connected to a server.</p>
		</div>
	{:else if usersStore.isLoading}
		<div class="flex items-center gap-2 text-muted-foreground">
			<div
				class="size-4 animate-spin rounded-full border-2 border-line-3 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading users...</span>
		</div>
	{:else if usersStore.error}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
			<p class="font-mono text-xs text-destructive">{usersStore.error}</p>
		</div>
	{:else if usersStore.users.length === 0}
		<div class="relative rounded-lg border border-border bg-card p-8">

			<div class="flex flex-col items-center justify-center gap-2 text-muted-foreground">
				<Users class="size-8 opacity-30" />
				<p class="font-mono text-xs">No users found</p>
				<button
					class="mt-2 rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
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
				<div class="group relative rounded-lg border border-border bg-card p-4">
					<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />

					<div class="flex items-start justify-between gap-3">
						<div class="flex items-center gap-3">
							<!-- Online indicator -->
							<span
								class="size-2.5 rounded-full flex-shrink-0 {user.is_online
									? 'bg-green'
									: 'bg-dim'}"
							></span>
							<div>
								<div class="flex items-center gap-2">
									<span class="font-mono text-sm text-foreground">{user.username}</span>
									{#if isCurrentUser}
										<span
											class="rounded bg-blue/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue"
										>
											You
										</span>
									{/if}
								</div>
								<div class="flex items-center gap-2 mt-1">
									<span
										class="rounded px-1.5 py-0.5 font-mono text-[9px] uppercase bg-chip text-muted-foreground"
									>
										{user.role}
									</span>
									<span class="font-mono text-[10px] text-muted-foreground">
										{formatLastLogin(user.last_login)}
									</span>
								</div>
							</div>
						</div>

						<!-- Actions -->
						{#if !isCurrentUser}
							<div class="flex items-center gap-1">
								<button
									class="p-1.5 hover:bg-hover-wash rounded transition-colors"
									title="Edit"
									onclick={() => (editingUser = user)}
								>
									<Edit class="w-3.5 h-3.5 text-muted-foreground" />
								</button>
								<button
									class="p-1.5 hover:bg-hover-wash rounded transition-colors"
									title="Sessions"
									onclick={() => (sessionsUser = user)}
								>
									<Key class="w-3.5 h-3.5 text-muted-foreground" />
								</button>
								<button
									class="p-1.5 hover:bg-destructive/10 rounded transition-colors"
									title="Delete"
									onclick={() => (deleteConfirmUser = user)}
								>
									<Trash2 class="w-3.5 h-3.5 text-destructive" />
								</button>
							</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<!-- Desktop/Tablet: Table Layout -->
		<div class="relative rounded-lg border border-border bg-card overflow-hidden">

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-16"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
							>Username</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-24"
							>Role</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-32"
							>Last Login</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-32"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each usersStore.users as user (user.id)}
						{@const isCurrentUser = usersStore.isCurrentUser(user)}
						<tr class="border-b border-border/30 hover:bg-hover-wash transition-colors">
							<!-- Status -->
							<td class="px-4 py-3">
								<div class="flex items-center gap-1.5">
									<span
										class="size-2 rounded-full {user.is_online ? 'bg-green' : 'bg-dim'}"
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
											class="rounded bg-blue/20 px-1.5 py-0.5 font-mono text-[9px] uppercase text-blue"
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
										<Shield class="w-3.5 h-3.5 text-yellow" />
									{:else}
										<Eye class="w-3.5 h-3.5 text-muted-foreground" />
									{/if}
									<span
										class="rounded px-1.5 py-0.5 font-mono text-[9px] uppercase bg-chip text-muted-foreground"
									>
										{user.role}
									</span>
								</div>
							</td>

							<!-- Last Login -->
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-muted-foreground">
									{formatLastLogin(user.last_login)}
								</span>
							</td>

							<!-- Actions -->
							<td class="px-4 py-3">
								{#if isCurrentUser}
									<span class="font-mono text-[10px] text-muted-foreground">-</span>
								{:else}
									<div class="flex items-center gap-1">
										<button
											class="p-1.5 hover:bg-hover-wash rounded transition-colors"
											title="Edit"
											onclick={() => (editingUser = user)}
										>
											<Edit class="w-3.5 h-3.5 text-muted-foreground" />
										</button>
										<button
											class="p-1.5 hover:bg-hover-wash rounded transition-colors"
											title="Sessions"
											onclick={() => (sessionsUser = user)}
										>
											<Key class="w-3.5 h-3.5 text-muted-foreground" />
										</button>
										<button
											class="p-1.5 hover:bg-destructive/10 rounded transition-colors"
											title="Delete"
											onclick={() => (deleteConfirmUser = user)}
										>
											<Trash2 class="w-3.5 h-3.5 text-destructive" />
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
	<div class="fixed inset-0 bg-black/70 z-40 flex items-center justify-center p-4">
		<div class="animate-luxury-fade-scale relative rounded-xl border border-border bg-popover w-full max-w-sm p-6 shadow-2xl">
			<LuxuryCornerSlivers />

			<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-4">Delete User</h3>
			<p class="font-mono text-sm text-text-2 mb-2">
				Are you sure you want to delete "{user.username}"?
			</p>
			{#if user.is_online}
				<p class="font-mono text-[10px] text-yellow mb-4">
					This user has active sessions that will be revoked.
				</p>
			{/if}
			<p class="font-mono text-[10px] text-muted-foreground mb-6">This action cannot be undone.</p>

			<div class="flex gap-2">
				<button
					class="flex-1 rounded bg-destructive px-4 py-2 font-mono text-sm text-destructive-foreground hover:bg-destructive/90 transition-colors"
					onclick={handleDeleteUser}
				>
					Delete
				</button>
				<button
					class="rounded border border-line-2 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
					onclick={() => (deleteConfirmUser = null)}
				>
					Cancel
				</button>
			</div>
		</div>
	</div>
{/if}
