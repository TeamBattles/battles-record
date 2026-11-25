# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The Activity log records downloads (queued, complete, failed) under a new Download filter, plus channels added or removed, channel errors, platform logins updated or expired, browser extension connections and failed library installs

### Fixed

- The dashboard's Recent Activity and the Activity log filled with "Unknown event: download_progress" during downloads, pushing real events out of the log
- Adding a channel could replace the Channels page with "500 Internal Error" when the server's channel-added notice arrived before its reply, listing the channel twice

## [2.0.1] - 2026-11-25

### Fixed

- The 2.0.0 app refused to connect to a 2.0.0 server with "Client version 2.0.0 is too new for this server"; the server now accepts any app in its own major version

## [2.0.0] - 2026-11-24

### Added

- First-run Welcome Screen introducing the app and letting new users pick their appearance before server setup; drag anywhere on it to move the window, and Settings can show it again
- Rounded app window corners on Windows and Linux, following the Rounding setting and squaring off when maximized
- Appearance settings shared with teambattles.gg and Battles Replay: background (Onyx, Navy, Graphite, Bone, System), accent (red, amber, cyan, lime), rounding (sharp, soft, round), density (comfortable, compact), high contrast and reduced motion
- Honeycomb (hex mesh) pattern on page headers, panel headers and dialog headers, with an accent ripple on page headers
- Battles Replay connects by approval: click Connect in the extension and allow it in Battles Record, with no pair code to copy
- Battles Record offers to start the local service when Battles Replay connects, with an option to always start it
- Download cards have an Open folder button, and an Open file button once the download is complete, which plays the file in your default app (local service only)

### Changed

- Desktop app restyled to the Arena Onyx design language used by teambattles.gg and Battles Replay: onyx surfaces, accent-colored primary actions, status color tokens and Bebas Neue page titles
- Cards, panels and dialogs now use rounded hairline borders with gradient corner accents instead of corner brackets
- Titlebar theme button is now a menu of the five backgrounds; the previous light/dark/system choice is migrated automatically
- Titlebar is slightly taller with a larger app icon
- Sidebar groups navigation into Record and Admin sections with an accent marker on the active page
- Daemon dependencies updated to their latest major versions (axum 0.8, reqwest 0.13, jsonwebtoken 12, rand 0.10, sysinfo 0.40, tokio-tungstenite 0.30, zip 9 and others)
- Daemon HTTPS now uses rustls instead of the system TLS library, so builds no longer need OpenSSL
- Desktop app updated to SvelteKit 3, Vite 8, TypeScript 6, Vitest 5, ESLint 10 and @lucide/svelte 1.x
- Docker images build with Rust 1.96
- Checkboxes are drawn in the app's theme instead of the operating system's native style
- Number fields have larger − / + buttons in place of the tiny native up/down arrows
- Dropdowns (page filters, Settings, cleanup tools, channel settings) use the app's own menu instead of the system dropdown, fixing their doubled border and the focus outline that reappeared after picking an option
- New Battles Record icon (scroll and quill) for the app window, taskbar, tray, installers, Store tiles, favicon and README
- The window, dock, taskbar and tray icons show the scroll and quill without its black background, filling more of the icon, and Linux uses the 256px image for the window icon instead of scaling up the 32px one
- Default window size raised to 900x800 so the Welcome Screen fits without scrolling; a window that would open larger than the screen is shrunk to fit
- The app listens for Battles Replay on 127.0.0.1:9555 (falling back to 9556 and 9557) whichever server it is viewing; extension downloads and channels go to this PC's local service, whose extension server now runs on a private port
- Settings > Browser Extension shows the extension port, the local service state, an option to start the service automatically and the approved browsers; the status bar's extension menu shows the same
- Messages to Battles Replay carry snake_case error codes instead of English sentences: `error`, `pair_failed`, `disconnected` and `connection_denied` have a `code`, download failures and summaries an `error_code`, and optional `detail` text is diagnostic only; `already_queued` carries `existing_id` and `quota_exceeded` its byte counts. The former `EXTRACT_FAILED`/`DOWNLOAD_FAILED`/`*_FAILED` codes are replaced by codes for the actual cause

### Removed

- "Show Corner Brackets" setting, replaced by the new appearance settings
- Unused `notify` and `@sveltejs/adapter-auto` dependencies
- Pair codes for the browser extension, and the library and downloads summary in the status bar's extension menu

### Fixed

- Cleanup Tools and Manual Token Entry cards no longer show square corners behind their rounded edges
- Battles Replay could not connect while the app was viewing a remote server, and the only way to pair it was a hard-to-find button in the status bar
- "Enter token manually" on a platform card now opens the manual token form for that platform instead of only changing its selection
- An empty `jwt_secret` now falls back to a generated temporary secret instead of signing tokens with an empty key
- Treat explicit empty extension cookie lists as anonymous for previews and downloads; explicit cookies bypass automatic yt-dlp account configuration, while omitted cookies retain daemon-local fallback.
- Adding a channel from Battles Replay that is already in the recording list now returns that channel's id (`existing_id` on `channel_exists`), so the extension can offer Remove for it
- Library installs and updates requested by Battles Replay that fail now send `library_install_failed` with a code (`download_failed`, `extract_failed`, `unsupported_platform`, `storage_error`) instead of only being logged
- After switching servers the app kept receiving live updates from the previous server, so pages showed stale status until they were reopened
- Settings > Library Management shows "Installing..." or "Updating..." while Battles Replay installs or updates a library, and updates when it finishes
- Downloads from Battles Replay are named with the title the extension sends rather than yt-dlp's. A Twitch recording sent as its bare playlist used to save as `index-dvr`, and the next one was skipped as already downloaded
- Downloading the Twitch recording of a stream that is still live started near the live edge, losing everything broadcast before it, and sat at 0% until the stream ended. It now starts from the beginning and follows the broadcast until it ends, and the download shows a Live badge with the size and speed so far instead of a percentage (`live: true` on `download_progress`)
- A finished download records the exact file yt-dlp wrote instead of the newest file in its channel folder, which could belong to another download of that channel

## [1.4.0] - 2026-03-26

### Added

#### Browser Extension Integration
- Extension WebSocket server with pairing, HMAC authentication, and rate-limited code validation
- Extension protocol: `add_channel`, `remove_channel`, `channels_state`, `uninstall_library` commands
- `channels_state` message sent on connect with full channel list (id, name, platform, status, profile image)
- `channel_added` / `channel_removed` broadcasts to all connected clients (extension and Tauri UI)
- Channel add from extension now persists to config, triggers immediate live check, and starts recording if live
- Channel remove from extension persists, stops active recording, and broadcasts to all clients
- Extension connection/disconnection events forwarded to Tauri UI in real time
- Pair code generation with `has_active_code()` status for UI display
- Message log viewer with JSON syntax highlighting and inline summary previews

#### Download Manager (yt-dlp)
- Full download queue with concurrency control, pause/resume/cancel/prioritize
- yt-dlp subprocess management with progress streaming and cookie forwarding
- Download storage statistics with per-channel breakdown
- Filtered download cleanup by age, channel, and platform with dry-run preview
- Duplicate URL rejection prevents queuing the same video twice
- Index reconciliation on startup: removes orphans, deduplicates, marks interrupted downloads as failed
- `find_existing_for_url()` to check if a URL was already downloaded
- Audio-only format detection skips incompatible post-processing (mp4 merge, thumbnail embedding)
- Better yt-dlp error reporting: shows actual stderr output instead of generic messages
- Enriched PATH for JS runtimes (Node.js, Bun, Deno) needed for YouTube signature extraction
- Empty cookie array now correctly falls back to persistent cookies instead of sending empty cookie file
- Download request includes title, quality, and requester name fields
- Default output template now includes resolution: `%(title)s [%(height)sp].%(ext)s`

#### Library Management
- FFmpeg update checking via BtbN GitHub API with 24-hour cache
- FFmpeg downloads switched from nightly master builds to latest stable releases
- FFmpeg version detection fixed: uses `-version` flag (not `--version`), strips BtbN `n` prefix
- Library uninstall: `DELETE /api/libraries/:name` endpoint and `uninstall_library` extension command
- Library install/update events now broadcast to all WebSocket and extension clients
- Per-library Install/Uninstall buttons in settings (replaces bulk "Install Libraries" button)
- System-wide library detection with info tooltip showing OS-specific uninstall instructions
- Bun added to library management display alongside yt-dlp and FFmpeg

#### Settings & Storage UI
- Library Management moved to its own settings section (out of Browser Extension section)
- Downloads directory configurable via `--downloads-dir` CLI flag and settings UI
- Downloads directory stored in Tauri settings (persists across restarts like recordings/library dirs)
- Download storage stats page with per-channel usage breakdown and progress bars
- Tabbed cleanup panel (Recordings / Downloads) with filtered cleanup support
- Extension connection status: "Disconnect" when connected, "Remove" when offline
- Microsoft Store MSIX packaging support via `winapp` CLI
- Automated Store submission workflow

#### Real-Time Updates
- `channel_added` / `channel_removed` WebSocket events update Channels page and Dashboard instantly
- Library status changes (install/update/uninstall) broadcast to all clients
- Dashboard refreshes channel count on add/remove events
- Profile cache cleaned up when channels are removed

### Changed
- Download progress bars use Svelte `Tween` animation instead of CSS transitions
- Toast notifications positioned above status bar
- Pause now works for queued downloads (not just actively downloading ones)
- FFmpeg update check failures are non-fatal (logged, doesn't block yt-dlp update check)
- API client handles HTTP 204 No Content responses
- Download removal is optimistic (removes from UI immediately before API call)
- Extension `AddChannel` handler validates platform, checks duplicates, persists config

### Fixed
- FFmpeg version always returned null (`--version` flag not recognized by FFmpeg)
- FFmpeg BtbN stable builds reported version as `n8.0` instead of `8.0`
- Downloads directory flashed default `./downloads` before showing configured path
- "Pause All" showed "download not found" for queued downloads
- yt-dlp YouTube extraction failed without JS runtime in PATH
- Audio-only downloads failed with "unsupported filetype for thumbnail embedding"
- Empty cookie array stripped authentication instead of falling back to persistent cookies
- Dashboard channel count not updated when channels added/removed via extension
- Profile cache leaked memory for removed channels
- Library status not refreshed after uninstall (path not shown until page reload)

## [1.3.1] - 2026-03-14

### Added
- Stale-while-revalidate pattern for page navigation — existing data shown immediately on return visits instead of loading spinners
- Stores track `_loadedServerId` to distinguish server switches (full reload) from same-server re-navigation (silent background refresh)

## [1.3.0] - 2026-03-13

### Changed
- Jellyfin seasons now use year-based grouping (Season 2024) instead of month-based (Season 01)
- Episode filenames now include ISO date for browsability (e.g. `xQc - S2024E001 - 2024-01-05 - Title.mp4`)
- Season poster shows year and episode count instead of specific date
- Episode thumb badge format changed from `S01E01` to `S2024E001`
- Improved code formatting and readability across daemon codebase
- Resolved all compiler warnings

### Fixed
- Episode thumbnails now use actual stream thumbnail URL (previously always blank)
- YouTube and Kick recordings are now exported to Jellyfin (previously silently skipped)
- Season metadata episode count now uses correct platform instead of hardcoded "twitch"
- Jellyfin export no longer skips when profile fetch fails (uses minimal fallback profile)

### Removed
- Unused `jellyfin/images.rs` module (superseded by `image_generator/`)

## [1.2.0] - 2026-02-09

### Added
- Automatic update checking for daemon and desktop app via GitHub Releases API (every 6 hours)
- Update notification banners in UI (dismissible, persisted across sessions)
- Client-daemon compatibility validation with `min_client_version` and `max_client_version` fields
- Incompatibility warning banner when client version is outside the supported range
- `check_for_updates` daemon config option (default: true)
- `BR_CHECK_FOR_UPDATES` Docker environment variable
- `update` field in `/api/status` response with latest version info

## [1.1.0] - 2026-02-09

### Fixed
- Buttons becoming non-interactable in release builds after using the server selector dropdown
- CSP blocking inline styles in release builds, preventing bits-ui/Floating UI from positioning dropdowns and menus correctly
- CSP blocking Tauri IPC protocol (`ipc.localhost`), forcing slower postMessage fallback
- CSP blocking remote server health checks and data URI images
- Titlebar drag region race condition caused by duplicate drag handlers (`data-tauri-drag-region` + manual `startDragging()`)

### Changed
- Tray icon "Exit" now uses the same close flow as the X button (instant UI hide, saved preference check, confirmation dialog)
- Removed `DropdownMenu.Portal` from server selector to avoid WebView2 hit-test corruption on Windows
- Added `select-none` to button base styles to prevent text selection on click
- Updated CSP to properly allow inline styles, Tauri IPC, remote connections, and data URIs
- Toast notifications overflowing when message contains long unbreakable strings (JSON errors, URLs)

## [1.0.0] - 2026-02-04

### Added

#### Core Recording Engine
- Multi-platform live stream recording for Twitch, YouTube, and Kick
- HLS segment downloading with concurrent segment fetches and priority queue (live edge = high, backfill = normal)
- Crash recovery by scanning for highest segment sequence number on restart
- Atomic writes: segments written to `.tmp` files, fsynced, then renamed for data integrity
- Configurable polling intervals for live detection and playlist fetching
- Stale stream detection with configurable timeout

#### Post-Processing
- Three output formats: `mp4_reencode` (full FFmpeg encode), `mp4_copy` (stream copy), `ts_concat` (segment concatenation)
- GPU-accelerated encoding support (NVIDIA NVENC, Intel Quick Sync, AMD AMF)
- Configurable encoding settings: codec, CRF, preset, audio bitrate
- Segment handling options: delete, concatenate, or keep original segments
- Background reconciliation worker that periodically scans for unprocessed recordings
- Per-channel post-processing overrides (format, segment handling, filename template)
- Concurrent processing with configurable job limit

#### Storage & Retention
- Global and per-channel storage quotas with warning thresholds
- Retention policies with configurable max age and minimum keep count
- Disk usage monitoring with configurable warning threshold
- Manual cleanup tools with dry-run mode and location targeting (recordings, library, or both)
- Automatic cleanup at configurable intervals

#### Scheduling & Filtering
- Per-channel schedule rules with day-of-week and time window support
- Timezone support for schedule evaluation (per-channel)
- Content filters: title includes/excludes, game includes/excludes, minimum viewer count
- Case-insensitive filter matching
- Skip event reporting (schedule, filter, quota) with reasons

#### Jellyfin Integration
- Jellyfin-compatible library export with TV show structure (platform/channel/season)
- NFO metadata generation for shows, seasons, and episodes
- Rich image generation with color palette extraction from channel profile images
- Show-level images: poster, banner, logo, fanart, landscape
- Season-level images: poster with date display
- Episode-level images: 4K thumbnail with metadata overlay
- Automatic profile and banner image fetching from platform APIs

#### Authentication & User Management
- JWT-based authentication with configurable session duration
- Admin and Viewer roles with appropriate permission boundaries
- Session management: view active sessions, revoke specific or all sessions per user
- Proactive token refresh (5 minutes before expiry) with refresh grace period
- Local-only mode for desktop sidecar (no auth required, localhost only)
- Graceful shutdown endpoint for local-only mode

#### Platform Authentication
- OAuth 2.0 flows for Twitch, YouTube, and Kick with bundled client credentials
- Custom OAuth client credential support for self-hosted deployments
- YouTube cookie-based authentication as OAuth alternative
- Automatic token refresh (10 minutes before expiry)
- Platform connection testing to verify auth status
- Manual token entry as fallback

#### Notifications
- Discord webhook notifications (stream start, end, error events)
- Telegram bot notifications (stream start, end, error events)
- Generic webhook notifications with custom headers and JSON payloads

#### Desktop Application (br-ui)
- Tauri 2 + SvelteKit + Svelte 5 desktop application
- Sidecar daemon management (start, stop, restart, status, logs, paths)
- System tray with daemon status display, quick actions, and minimize-to-tray
- Close prevention when daemon is running (confirms before exit)
- Custom titlebar with light/dark theme toggle
- Auto-reconnect on daemon connection loss
- Session expired modal with seamless re-authentication
- Dashboard with live stats, active recordings, recent activity, and system health
- Channel management with list/grid views, quick actions, and detail panels
- Recordings browser with status filtering, sorting, and batch operations
- Activity log with real-time streaming, filters, and export
- Storage management with usage visualization, per-channel breakdown, and cleanup tools
- Schedule editor with visual timeline and timezone selection
- Settings page for server, post-processing, storage, notifications, and appearance
- Platform auth page with OAuth flow, cookie upload, and connection testing
- User management page (admin only) with session management
- YouTube dependency installer (Bun and yt-dlp auto-install)
- Responsive design: mobile, tablet, and desktop breakpoints
- Deep link handler for OAuth callbacks

#### REST API & WebSocket
- Full CRUD API for channels, recordings, users, and configuration
- Channel image management (profile/banner upload, platform fetch)
- Post-processing configuration endpoint
- Storage statistics and cleanup endpoints
- System dependencies endpoint
- WebSocket at `/api/events` for real-time event streaming
- Events: channel status, recording lifecycle, processing progress, skip events, quota changes, platform auth updates, disk warnings, config reload

#### Docker Deployment
- Standard Docker image (`latest`) with Debian slim + FFmpeg
- NVIDIA GPU image (`nvidia`) with CUDA runtime + FFmpeg NVENC support
- Environment variable configuration with `docker-entrypoint.sh` config generation
- Docker Compose support with volume mounts for recordings, library, images, and config
- Health check endpoint at `/health`

#### Configuration
- TOML configuration with hot-reload via filesystem watcher
- Separate channels file for Docker persistence across container restarts
- CLI overrides for host, port, and config path
- Configurable log level and optional log file output
