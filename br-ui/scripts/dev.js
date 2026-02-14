import { spawn, spawnSync } from 'child_process';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { writeFileSync, unlinkSync, existsSync } from 'fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const brUiDir = join(__dirname, '..');
const configOverridePath = join(brUiDir, 'src-tauri', '.dev-override.json');

// Strip ANSI escape codes from text
function stripAnsi(text) {
	return text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
}

function cleanup() {
	if (existsSync(configOverridePath)) {
		try {
			unlinkSync(configOverridePath);
		} catch {
			// Ignore cleanup errors
		}
	}
}

// Kill a process and its entire tree (needed on Windows where shell: true creates wrapper processes)
function killProcessTree(proc) {
	if (!proc || !proc.pid) return;

	if (process.platform === 'win32') {
		// On Windows, use taskkill with /T to kill the entire process tree
		// Must use spawnSync so the kill completes before the script exits
		try {
			spawnSync('taskkill', ['/PID', proc.pid.toString(), '/T', '/F'], { stdio: 'ignore' });
		} catch {
			// Fallback to regular kill
			proc.kill();
		}
	} else {
		// On Unix, kill the process group
		try {
			process.kill(-proc.pid, 'SIGTERM');
		} catch {
			proc.kill();
		}
	}
}

async function main() {
	process.chdir(brUiDir);

	let vite = null;
	let tauri = null;
	let resolved = false;

	function cleanupAll() {
		killProcessTree(vite);
		killProcessTree(tauri);
		cleanup();
	}

	// Start Vite dev server and parse its output to find the actual port
	const portPromise = new Promise((resolve, reject) => {
		// Use single command string to avoid deprecation warning (DEP0190)
		vite = spawn('npx vite dev', {
			shell: true,
			cwd: brUiDir,
			stdio: ['inherit', 'pipe', 'pipe']
		});

		let output = '';
		const timeout = setTimeout(() => {
			if (!resolved) {
				console.error('\n  Debug: Full output received:\n', stripAnsi(output));
				reject(new Error('Timeout waiting for Vite to start'));
			}
		}, 30000);

		function checkForPort(text) {
			if (resolved) return;
			output += text;
			// Strip ANSI codes before matching
			const clean = stripAnsi(output);
			// Try multiple patterns to match Vite's URL output
			let match = clean.match(/Local:\s+http:\/\/localhost:(\d+)/);
			if (!match) {
				match = clean.match(/http:\/\/localhost:(\d+)\/?/);
			}
			if (match) {
				resolved = true;
				clearTimeout(timeout);
				resolve(parseInt(match[1], 10));
			}
		}

		vite.stdout.on('data', (data) => {
			const text = data.toString();
			process.stdout.write(text);
			checkForPort(text);
		});

		vite.stderr.on('data', (data) => {
			const text = data.toString();
			process.stderr.write(text);
			checkForPort(text);
		});

		vite.on('error', (err) => {
			clearTimeout(timeout);
			reject(err);
		});

		vite.on('exit', (code) => {
			if (code !== 0 && code !== null) {
				clearTimeout(timeout);
				reject(new Error(`Vite exited with code ${code}`));
			}
		});
	});

	console.log('  Starting Vite...\n');

	let port;
	try {
		port = await portPromise;
	} catch (err) {
		console.error('Failed to start Vite:', err.message);
		cleanupAll();
		process.exit(1);
	}

	const devUrl = `http://localhost:${port}`;
	console.log(`\n  Detected Vite on port ${port}, starting Tauri...\n`);

	// Write config override with the actual port Vite is using
	const configOverride = {
		build: {
			devUrl: devUrl
		}
	};
	writeFileSync(configOverridePath, JSON.stringify(configOverride, null, 2));

	// Start Tauri dev with config override
	// Use single command string to avoid deprecation warning (DEP0190)
	tauri = spawn(`npx tauri dev --config src-tauri/.dev-override.json`, {
		stdio: 'inherit',
		shell: true,
		cwd: brUiDir
	});

	tauri.on('error', (err) => {
		console.error('Tauri error:', err);
		cleanupAll();
		process.exit(1);
	});

	tauri.on('exit', (code) => {
		cleanupAll();
		process.exit(code || 0);
	});

	process.on('SIGINT', cleanupAll);
	process.on('SIGTERM', cleanupAll);
}

main().catch((err) => {
	console.error(err);
	cleanup();
	process.exit(1);
});
