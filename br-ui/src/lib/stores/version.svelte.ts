import { browser } from '$app/environment';
import { isNewerVersion, isCompatibleMin, isCompatibleMax, checkLatestRelease, CHECK_INTERVAL_MS } from '$lib/utils/version';

const DISMISSED_KEY = 'br-dismissed-versions';

type BannerType = 'incompatible' | 'daemon-update' | 'ui-update';

class VersionStore {
	appVersion = $state<string | null>(null);

	// UI update state (from GitHub check)
	latestUIVersion = $state<string | null>(null);
	uiUpdateAvailable = $state(false);
	uiReleaseUrl = $state<string | null>(null);

	// Daemon update state (from status API)
	daemonUpdateAvailable = $state(false);
	daemonLatestVersion = $state<string | null>(null);
	daemonReleaseUrl = $state<string | null>(null);

	// Compatibility state
	isCompatible = $state(true);
	incompatibleReason = $state<string | null>(null);
	minClientVersion = $state<string | null>(null);

	private dismissedVersions = $state(new Set<string>());
	private checkInterval: ReturnType<typeof setInterval> | null = null;

	get activeBanner(): BannerType | null {
		if (!this.isCompatible) return 'incompatible';
		if (this.daemonUpdateAvailable && !this.isDismissed(`daemon-${this.daemonLatestVersion}`))
			return 'daemon-update';
		if (this.uiUpdateAvailable && !this.isDismissed(`ui-${this.latestUIVersion}`))
			return 'ui-update';
		return null;
	}

	async init() {
		if (!browser) return;

		this.loadDismissed();

		try {
			const { getVersion } = await import('@tauri-apps/api/app');
			this.appVersion = await getVersion();
		} catch {
			// Not running in Tauri
			this.appVersion = null;
		}

		// Start periodic GitHub check for UI updates
		await this.checkUIUpdate();
		this.checkInterval = setInterval(() => this.checkUIUpdate(), CHECK_INTERVAL_MS);
	}

	destroy() {
		if (this.checkInterval) {
			clearInterval(this.checkInterval);
			this.checkInterval = null;
		}
	}

	async checkUIUpdate() {
		if (!this.appVersion) return;

		const release = await checkLatestRelease();
		if (!release) return;

		this.latestUIVersion = release.version;
		this.uiReleaseUrl = release.url;
		this.uiUpdateAvailable = isNewerVersion(this.appVersion, release.version);
	}

	/** Called after dashboard loads status to check client-daemon compatibility. */
	checkCompatibility(minClientVersion: string, maxClientVersion: string) {
		if (!this.appVersion) {
			this.isCompatible = true;
			return;
		}

		this.minClientVersion = minClientVersion;

		const meetsMin = isCompatibleMin(this.appVersion, minClientVersion);
		const meetsMax = isCompatibleMax(this.appVersion, maxClientVersion);

		if (!meetsMin) {
			this.isCompatible = false;
			this.incompatibleReason = `Client version ${this.appVersion} is too old. Minimum required: ${minClientVersion}`;
		} else if (!meetsMax) {
			this.isCompatible = false;
			this.incompatibleReason = `Client version ${this.appVersion} is too new for this server. Maximum supported: ${maxClientVersion}`;
		} else {
			this.isCompatible = true;
			this.incompatibleReason = null;
		}
	}

	/** Update daemon version info from status API response. */
	setDaemonUpdateInfo(update: { update_available: boolean; latest_version: string | null; release_url: string | null } | undefined) {
		if (!update) return;
		this.daemonUpdateAvailable = update.update_available;
		this.daemonLatestVersion = update.latest_version;
		this.daemonReleaseUrl = update.release_url;
	}

	dismissBanner(versionKey: string) {
		this.dismissedVersions = new Set([...this.dismissedVersions, versionKey]);
		this.saveDismissed();
	}

	isDismissed(versionKey: string): boolean {
		return this.dismissedVersions.has(versionKey);
	}

	private loadDismissed() {
		try {
			const stored = localStorage.getItem(DISMISSED_KEY);
			if (stored) {
				this.dismissedVersions = new Set(JSON.parse(stored));
			}
		} catch {
			// Ignore parse errors
		}
	}

	private saveDismissed() {
		try {
			localStorage.setItem(DISMISSED_KEY, JSON.stringify([...this.dismissedVersions]));
		} catch {
			// Ignore storage errors
		}
	}
}

export const versionStore = new VersionStore();
