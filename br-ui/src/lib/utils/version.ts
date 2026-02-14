const GITHUB_RELEASES_URL =
	'https://api.github.com/repos/TeamBattles/battles-record/releases/latest';

export const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000; // 6 hours

function parseSemver(version: string): [number, number, number] {
	const v = version.replace(/^v/, '');
	const parts = v.split('.');
	return [
		parseInt(parts[0] || '0', 10) || 0,
		parseInt(parts[1] || '0', 10) || 0,
		parseInt(parts[2] || '0', 10) || 0
	];
}

/** Returns true if `latest` is a newer semver than `current`. */
export function isNewerVersion(current: string, latest: string): boolean {
	const [cMaj, cMin, cPat] = parseSemver(current);
	const [lMaj, lMin, lPat] = parseSemver(latest);

	if (lMaj !== cMaj) return lMaj > cMaj;
	if (lMin !== cMin) return lMin > cMin;
	return lPat > cPat;
}

/** Returns true if `clientVersion` >= `minVersion`. */
export function isCompatibleMin(clientVersion: string, minVersion: string): boolean {
	return !isNewerVersion(clientVersion, minVersion);
}

/** Returns true if `clientVersion` <= `maxVersion`. */
export function isCompatibleMax(clientVersion: string, maxVersion: string): boolean {
	return !isNewerVersion(maxVersion, clientVersion);
}

/** Fetch the latest release info from GitHub. */
export async function checkLatestRelease(): Promise<{
	version: string;
	url: string;
} | null> {
	try {
		const resp = await fetch(GITHUB_RELEASES_URL, {
			headers: { Accept: 'application/vnd.github.v3+json' }
		});
		if (!resp.ok) return null;

		const data = await resp.json();
		const version = (data.tag_name as string)?.replace(/^v/, '');
		const url = data.html_url as string;
		if (!version || !url) return null;

		return { version, url };
	} catch {
		return null;
	}
}
