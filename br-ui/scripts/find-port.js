import { createServer } from 'net';
import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function findAvailablePort(startPort) {
	for (let port = startPort; port < startPort + 100; port++) {
		const available = await isPortAvailable(port);
		if (available) return port;
	}
	throw new Error(`No available port found in range ${startPort}-${startPort + 99}`);
}

function isPortAvailable(port) {
	return new Promise((resolve) => {
		const server = createServer();
		server.once('error', () => resolve(false));
		server.once('listening', () => {
			server.close();
			resolve(true);
		});
		server.listen(port, '127.0.0.1');
	});
}

async function main() {
	const port = await findAvailablePort(5173);

	// Write config override for Tauri
	const configPath = join(__dirname, '..', 'src-tauri', '.dev-config.json');
	writeFileSync(configPath, JSON.stringify({
		build: { devUrl: `http://localhost:${port}` }
	}));

	// Output port for the calling process
	console.log(port);
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
