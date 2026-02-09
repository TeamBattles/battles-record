import { execSync } from 'child_process';
import { copyFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..', '..');
const binariesDir = join(__dirname, '..', 'src-tauri', 'binaries');

// Get target triples for the platform
// On Windows, we need both MSVC and GNU variants since Tauri may use either
function getTargetTriples() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === 'win32') {
    // Tauri may use either MSVC or GNU toolchain on Windows
    return ['x86_64-pc-windows-msvc', 'x86_64-pc-windows-gnu'];
  } else if (platform === 'darwin') {
    return [arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'];
  } else {
    return ['x86_64-unknown-linux-gnu'];
  }
}

const targetTriples = getTargetTriples();
const isWindows = process.platform === 'win32';
const ext = isWindows ? '.exe' : '';

// Paths
const sourceBinary = join(repoRoot, 'target', 'release', `br-daemon${ext}`);

console.log(`Building br-daemon...`);

// Build br-daemon in release mode
execSync('cargo build --release -p br-daemon', {
  cwd: repoRoot,
  stdio: 'inherit'
});

// Ensure binaries directory exists
if (!existsSync(binariesDir)) {
  mkdirSync(binariesDir, { recursive: true });
}

// Copy binary for each target triple
for (const triple of targetTriples) {
  const destBinary = join(binariesDir, `br-daemon-${triple}${ext}`);
  console.log(`Copying to ${destBinary}`);
  copyFileSync(sourceBinary, destBinary);
}

console.log('Sidecar binaries ready!');
