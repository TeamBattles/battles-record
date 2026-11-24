const WELCOME_STORAGE_KEY = 'br-welcome-complete';

/**
 * Whether the first-run Welcome Screen has been finished. Kept in localStorage beside the
 * appearance preferences it collects, so it survives restarts without waiting on the async
 * Tauri settings file.
 */
class WelcomeStore {
	isComplete = $state(false);

	init() {
		this.isComplete = read() === '1';
	}

	complete() {
		this.isComplete = true;
		write('1');
	}

	/** Lets Settings show the Welcome Screen again; the root layout reacts immediately. */
	reset() {
		this.isComplete = false;
		write(null);
	}
}

function read(): string | null {
	try {
		return localStorage.getItem(WELCOME_STORAGE_KEY);
	} catch {
		return null;
	}
}

function write(value: string | null) {
	try {
		if (value === null) localStorage.removeItem(WELCOME_STORAGE_KEY);
		else localStorage.setItem(WELCOME_STORAGE_KEY, value);
	} catch {
		// Unavailable storage only means the Welcome Screen may show again next launch.
	}
}

export const welcomeStore = new WelcomeStore();
