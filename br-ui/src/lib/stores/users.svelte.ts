import { api } from '$lib/api';
import type { User, Session, CreateUserRequest, UpdateUserRequest, UserRole } from '$lib/api/types';
import { toastStore } from './toast.svelte';

class UsersStore {
	users = $state<User[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Selection
	selectedUserId = $state<number | null>(null);

	// Current user (from auth)
	currentUsername = $state<string | null>(null);

	get selectedUser() {
		return this.users.find((u) => u.id === this.selectedUserId) ?? null;
	}

	get userCount() {
		return this.users.length;
	}

	get onlineCount() {
		return this.users.filter((u) => u.is_online).length;
	}

	setCurrentUsername(username: string | null) {
		this.currentUsername = username;
	}

	isCurrentUser(user: User): boolean {
		return user.username === this.currentUsername;
	}

	// Track which server's data we have for stale-while-revalidate
	private _loadedServerId: string | null = null;

	async load(serverId?: string) {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.users.length > 0;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) this.users = [];
		}
		this.error = null;
		try {
			this.users = await api.getUsers();
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load users';
		} finally {
			this.isLoading = false;
		}
	}

	async createUser(data: CreateUserRequest): Promise<boolean> {
		try {
			const user = await api.createUser(data);
			this.users = [...this.users, user];
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to create user';
			return false;
		}
	}

	async updateUser(id: number, data: UpdateUserRequest): Promise<boolean> {
		try {
			const updated = await api.updateUser(id, data);
			const index = this.users.findIndex((u) => u.id === id);
			if (index !== -1) {
				this.users[index] = updated;
			}
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to update user';
			return false;
		}
	}

	async deleteUser(id: number): Promise<boolean> {
		try {
			await api.deleteUser(id);
			this.users = this.users.filter((u) => u.id !== id);
			if (this.selectedUserId === id) {
				this.selectedUserId = null;
			}
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to delete user';
			return false;
		}
	}

	selectUser(id: number | null) {
		this.selectedUserId = id;
	}
}

class UserSessionsStore {
	sessions = $state<Session[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);
	userId = $state<number | null>(null);

	async load(userId: number) {
		this.userId = userId;
		this.isLoading = true;
		this.error = null;
		try {
			this.sessions = await api.getUserSessions(userId);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load sessions';
		} finally {
			this.isLoading = false;
		}
	}

	async revokeSession(sessionId: string): Promise<boolean> {
		if (this.userId === null) return false;
		try {
			await api.revokeUserSession(this.userId, sessionId);
			this.sessions = this.sessions.filter((s) => s.id !== sessionId);
			toastStore.success('Session revoked');
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to revoke session';
			return false;
		}
	}

	async revokeAllSessions(): Promise<boolean> {
		if (this.userId === null) return false;
		try {
			const count = await api.revokeAllUserSessions(this.userId);
			this.sessions = [];
			toastStore.success(`Revoked ${count} session${count !== 1 ? 's' : ''}`);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to revoke sessions';
			return false;
		}
	}

	clear() {
		this.sessions = [];
		this.userId = null;
		this.error = null;
	}
}

export const usersStore = new UsersStore();
export const userSessionsStore = new UserSessionsStore();
