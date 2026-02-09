export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration: number;
}

class ToastStore {
	toasts = $state<Toast[]>([]);

	add(type: ToastType, message: string, duration = 5000): string {
		const id = crypto.randomUUID();
		this.toasts = [...this.toasts, { id, type, message, duration }];

		if (duration > 0) {
			setTimeout(() => this.dismiss(id), duration);
		}

		return id;
	}

	dismiss(id: string) {
		this.toasts = this.toasts.filter((t) => t.id !== id);
	}

	success(message: string): string {
		return this.add('success', message, 5000);
	}

	error(message: string): string {
		return this.add('error', message, 8000);
	}

	info(message: string): string {
		return this.add('info', message, 5000);
	}

	warning(message: string): string {
		return this.add('warning', message, 6000);
	}
}

export const toastStore = new ToastStore();
