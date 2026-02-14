# Browser Extension Integration - Battles Replay

This document specifies the changes required in the Battles Replay browser extension to integrate with the battles-record desktop app for yt-dlp downloads.

**Companion spec:** `docs/superpowers/specs/2026-03-24-ytdlp-extension-bridge-design.md` (covers daemon-side architecture, merge/alias system, and full protocol reference)

---

## 1. Overview

The extension connects to the battles-record daemon via WebSocket on localhost. When a user visits a supported site (YouTube, Instagram, TikTok, etc.), the extension can trigger downloads that are executed by the daemon using yt-dlp. Download progress is streamed back in real-time.

---

## 2. New Modules

### desktop-bridge.js

WebSocket client with auto-reconnect and port fallback.

**Connection logic:**

1. On extension startup, check `chrome.storage.local` for cached port and pairing token
2. If cached port exists, try that first. Otherwise try ports in order: 9555, 9556, 9557
3. On WebSocket open, send `hello` with token (if stored)
4. If `requires_pairing: true` in response, prompt user for pair code + identifier
5. On successful pair, store token and port in `chrome.storage.local`
6. Auto-reconnect with exponential backoff (1s, 2s, 4s, 8s, max 30s)
7. On `port_changed` message, store new port and reconnect to it

**Key methods:**

- `connect()` - Establish WebSocket connection with port fallback
- `request(msg, timeoutMs)` - Request/response pattern with timeout (default 30s)
- `send(msg)` - Fire-and-forget command
- `subscribe(listener)` - Subscribe to all incoming events, returns unsubscribe function
- `disconnect()` - Send `unpair` message to daemon (removes pairing server-side), then close WebSocket and clear stored token
- `get connected()` - Current connection state

### desktop-protocol.js

Message type constants and serialization helpers. See the full protocol in the companion spec, Section 6.

### site-detector.js

URL pattern matching for supported sites. Curated list of ~20 major sites:

```javascript
const SUPPORTED_SITES = [
  { id: 'youtube', pattern: /youtube\.com\/watch|youtu\.be\//, name: 'YouTube' },
  { id: 'instagram', pattern: /instagram\.com\/(p|reel|tv)\//, name: 'Instagram' },
  { id: 'tiktok', pattern: /tiktok\.com\/@.*\/video\//, name: 'TikTok' },
  { id: 'twitter', pattern: /x\.com\/.*\/status\/|twitter\.com\/.*\/status\//, name: 'X/Twitter' },
  { id: 'reddit', pattern: /reddit\.com\/.*\/comments\//, name: 'Reddit' },
  { id: 'facebook', pattern: /facebook\.com\/.*\/videos\/|fb\.watch\//, name: 'Facebook' },
  { id: 'vimeo', pattern: /vimeo\.com\/\d+/, name: 'Vimeo' },
  { id: 'dailymotion', pattern: /dailymotion\.com\/video\//, name: 'Dailymotion' },
  { id: 'soundcloud', pattern: /soundcloud\.com\//, name: 'SoundCloud' },
  { id: 'bilibili', pattern: /bilibili\.com\/video\//, name: 'Bilibili' },
  { id: 'rumble', pattern: /rumble\.com\//, name: 'Rumble' },
  { id: 'odysee', pattern: /odysee\.com\//, name: 'Odysee' },
  { id: 'streamable', pattern: /streamable\.com\//, name: 'Streamable' },
  { id: 'imgur', pattern: /imgur\.com\//, name: 'Imgur' },
  { id: 'pinterest', pattern: /pinterest\.com\/pin\//, name: 'Pinterest' },
  { id: 'linkedin', pattern: /linkedin\.com\/.*\/(posts|feed)\//, name: 'LinkedIn' },
  { id: 'threads', pattern: /threads\.net\//, name: 'Threads' },
  { id: 'twitch_clip', pattern: /clips\.twitch\.tv\/|twitch\.tv\/.*\/clip\//, name: 'Twitch Clip' },
  { id: 'kick_clip', pattern: /kick\.com\/.*\/clips\//, name: 'Kick Clip' },
];
```

Any URL can also be sent manually - yt-dlp decides if it's supported.

---

## 3. New UI Components

### DesktopConnectionBadge.svelte

Small indicator shown in the extension UI (e.g., near the search bar or in the header).

**States:**

| State | Display |
|-------|---------|
| Connected | Green dot + "Connected" or app version |
| Connecting | Yellow dot + "Connecting..." |
| Disconnected | Red dot + "Disconnected" |
| Not paired | "Connect to Battles Record" link |

### RemoteDownloadPanel.svelte

Format picker and download trigger. Shown when the extension detects a supported site and the desktop app is connected.

**Behavior:**

1. On extension open (supported site detected), send `extract_info` immediately
2. Show spinner on the download button while waiting
3. On `info_result`, populate format picker (resolution, codec, file size estimate)
4. User selects format and clicks Download
5. If user clicks before extract_info completes, queue the download (starts after info arrives)

**Format picker display:**

- Resolution (1080p, 720p, 480p, etc.)
- File type (MP4, WEBM, M4A for audio)
- Estimated file size
- Codec info (AVC, VP9, etc.)
- "Audio only" option

### DesktopSettings.svelte

New section in the extension's settings page.

**Contents:**

- Connection status indicator
- Custom port input (override default 9555)
- Paired app info (identifier, connected since)
- **[Disconnect]** button (clears stored token, closes WebSocket)
- Default download format preference
- Toggle for proactive extract_info on supported sites

---

## 4. Download Integration

### Extension Downloads Tab

Remote (yt-dlp) downloads appear alongside existing local (HLS) downloads in the same Downloads tab.

**Per-download display:**

- Title and thumbnail (from extract_info)
- Platform badge ("YouTube", "Instagram", etc.)
- Progress bar with percentage, speed, ETA
- Status indicator (downloading, queued, paused, complete, failed)
- Pause/Resume and Cancel buttons
- Priority button (moves to front of queue)
- Error state with **[Update yt-dlp]** action if applicable

**Controls work identically to local downloads** - pause, resume, cancel, prioritize all send commands via WebSocket with `id` fields for request/response correlation. The daemon handles execution.

**Download command fields:** The `download` command must include a `source_platform` field (e.g., `"youtube"`, `"instagram"`) derived from the site detector match. This determines the platform subfolder in the downloads directory on the daemon side (`{downloads_dir}/{platform}/{channel}/`). Downloads are stored separately from stream recordings.

**`channel_name` can be null:** If no channel name is available, the extension sends `channel_name: null`. The daemon derives a fallback from the URL domain (e.g., `"youtube.com"`). The `options` field is optional - when omitted, the daemon uses defaults (embed_thumbnail: true, embed_metadata: true).

**Format selection timeout:** If the user doesn't select a format within 10 minutes of `extract_info` completing, the daemon auto-starts the download with the configured default format. The extension should show a countdown or warning as the timeout approaches.

### Completed downloads

Show filename and path. No "Show in folder" action since the extension can't open local folders - just display the path for reference.

---

## 5. Proactive Extract Info

When the user opens the extension on a supported site:

1. Extension sends `extract_info` for ANY non-Twitch/Kick URL when connected (not gated on site-detector match - yt-dlp decides support)
2. The `extract_info` message includes `auto_start: false` to prevent the daemon's format selection timeout from auto-starting downloads
3. Download button is disabled (not clickable) while waiting for format info
4. On result, format picker populates and button enables
5. On failure or timeout (30s), show error with retry

If desktop is not connected, the connection badge is hidden entirely. Users discover the feature through the Browse tab "Other" mode and VodPlayer banners.

---

## 6. Cookie Forwarding

For authenticated downloads (YouTube Premium, private videos, etc.):

```javascript
async function getCookiesForUrl(url) {
  const cookies = await chrome.cookies.getAll({ url });
  return cookies.map(c => ({
    domain: c.domain,
    path: c.path,
    secure: c.secure,
    expirationDate: c.expirationDate || 0,
    httpOnly: c.httpOnly,
    name: c.name,
    value: c.value,
  }));
}
```

Full cookie objects (not just `name=value` strings) are sent as a JSON array in the `download` command. The daemon needs domain, path, secure, and expiration fields to convert to Netscape cookie file format for yt-dlp's `--cookies` flag.

---

## 7. Manifest Changes

```json
{
  "permissions": ["storage", "downloads", "offscreen", "tabs", "cookies", "alarms"]
}
```

Additions: `cookies` for cookie forwarding, `alarms` for WebSocket keepalive timer.

No new `host_permissions` needed - `ws://127.0.0.1` is allowed from MV3 service workers.

---

## 8. Library Management UI

When the desktop app reports libraries are not installed (via `hello` response):

1. Show a prominent banner: "Download Required Libraries"
2. Clicking sends `install_libraries` via WebSocket
3. Show download progress for each library
4. On completion, enable download functionality

When an update is available:

1. Subtle indicator near the connection badge
2. In settings: "Update available" with version info
3. On download failure with `update_available: true`: actionable error "yt-dlp may be outdated. [Update now]"

---

## 9. Port Change Handling

If the daemon sends `port_changed { new_port }`:

1. Store new port in `chrome.storage.local`
2. Close current WebSocket connection
3. Reconnect to new port using existing pairing token
4. Normal reconnect logic handles any transient failures during switchover

If the user sets a custom port in extension settings:

1. Store custom port in `chrome.storage.local`
2. Reconnect to custom port
3. Custom port takes priority over cached port from daemon

---

## 10. Error Handling

| Scenario | Extension Behavior |
|----------|-------------------|
| Desktop app not running | "Connect to Battles Record" prompt, auto-reconnect with exponential backoff (1s, 2s, 4s, 8s, max 30s) |
| Libraries not installed | "Download Required Libraries" banner |
| Pair code expired (5-min TTL) | Request new pair code automatically |
| Pair code wrong 5 times | Code invalidated, new code generated. After 3 invalidated codes, 5-min cooldown. Show appropriate message. |
| Extract info timeout | Show error, offer retry button |
| Download failed | Show error message from daemon |
| Download failed + outdated yt-dlp | Show error + "Update yt-dlp" action |
| Quota exceeded | Show quota warning with usage details |
| WebSocket disconnected mid-download | Show disconnected state, auto-reconnect, downloads continue on daemon |
| Port changed | Auto-reconnect to new port (daemon sends 2s before switching) |
| Daemon shutting down | Receive `disconnected { reason: "Server shutting down" }`, show status, auto-reconnect attempts begin |

---

## 11. Add Channel to Recording List

The extension can add channels to the daemon's live stream recording list. This is useful when a user is watching a stream and wants the daemon to start monitoring and recording it.

### Protocol

**Extension sends:**
```json
{ "type": "add_channel", "id": "req_010", "name": "xqc", "platform": "twitch" }
```

**Daemon responds (success):**
```json
{ "type": "channel_added", "id": "req_010", "channel_id": "uuid", "name": "xqc", "platform": "twitch" }
```

**Daemon responds (duplicate):**
```json
{ "type": "error", "id": "req_010", "code": "CHANNEL_EXISTS", "message": "Channel 'xqc' already exists for twitch" }
```

**Daemon responds (invalid platform):**
```json
{ "type": "error", "id": "req_010", "code": "INVALID_PLATFORM", "message": "Unsupported platform: tiktok. Use twitch, youtube, or kick." }
```

### Behavior

- The extension auto-detects the platform from the current URL (Twitch, Kick, or YouTube)
- Channel is created with defaults: enabled, "best" quality, no schedule/filters
- The channel is **persisted to the daemon's config file** (survives restarts)
- The daemon immediately checks if the channel is live and starts recording if so
- The user can customize the channel later in the Tauri app settings
- Only `twitch`, `youtube`, and `kick` platforms are supported for live recording

### Broadcasts

When a channel is added (from the extension OR from the Tauri app), the daemon broadcasts to all connected extensions:
```json
{ "type": "channel_added", "id": "", "channel_id": "uuid", "name": "xqc", "platform": "twitch" }
```

The extension should listen for unsolicited `channel_added` broadcasts to update its channel list when channels are added from the app side or by another extension client.

---

## 12. Remove Channel from Recording List

The extension can remove channels from the daemon's live stream recording list, and the daemon notifies connected extensions when a channel is removed from the app.

### Protocol

**Extension sends (remove a channel):**
```json
{ "type": "remove_channel", "id": "req_011", "channel_id": "550e8400-e29b-41d4-a716-446655440000" }
```

**Daemon responds (success):**
```json
{ "type": "channel_removed", "id": "req_011", "channel_id": "550e8400-...", "name": "xqc", "platform": "twitch" }
```

**Daemon responds (invalid UUID):**
```json
{ "type": "error", "id": "req_011", "code": "INVALID_ID", "message": "Invalid channel ID: not-a-uuid" }
```

**Daemon responds (not found):**
```json
{ "type": "error", "id": "req_011", "code": "CHANNEL_NOT_FOUND", "message": "No channel found with ID: 550e8400-..." }
```

**Daemon broadcasts (channel deleted from app):**

When a channel is deleted via the Tauri app or REST API, the daemon broadcasts to all connected extensions:
```json
{ "type": "channel_removed", "id": "", "channel_id": "550e8400-...", "name": "xqc", "platform": "twitch" }
```

Note: Broadcast notifications have an empty `id` field since they are not responses to a request.

### Behavior

- The extension sends the channel's UUID (received from `channel_added` or from the `channels_state` list)
- The removal is **persisted to the daemon's config file** (survives restarts)
- If the channel has an active recording, it is stopped automatically
- The extension should listen for unsolicited `channel_removed` broadcasts to update its UI when channels are removed from the app side
- To distinguish request responses from broadcasts, check whether the `id` field matches a pending request

---

## 13. Initial Channel List

After authentication, the daemon automatically sends the current list of recording channels (alongside the `queue_state` message). No request is needed.

### Protocol

**Daemon sends (automatically after auth):**
```json
{
  "type": "channels_state",
  "channels": [
    {
      "channel_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "xqc",
      "platform": "twitch",
      "enabled": true,
      "status": "recording",
      "profile_image_url": "https://static-cdn.jtvnw.net/..."
    },
    {
      "channel_id": "660f9500-...",
      "name": "kai_cenat",
      "platform": "kick",
      "enabled": true,
      "status": "offline",
      "profile_image_url": null
    }
  ]
}
```

### Fields per channel

| Field | Type | Description |
|-------|------|-------------|
| `channel_id` | string | UUID, use this for `remove_channel` |
| `name` | string | Channel name |
| `platform` | string | `"twitch"`, `"youtube"`, or `"kick"` |
| `enabled` | boolean | Whether the daemon monitors this channel |
| `status` | string | `"offline"`, `"live"`, or `"recording"` |
| `profile_image_url` | string or null | Profile image URL if available |

### Keeping the list up to date

The initial `channels_state` provides the snapshot at connection time. After that, listen for these broadcast messages to keep the list current:

- `channel_added` - a new channel was added (from extension or app)
- `channel_removed` - a channel was removed (from extension or app)
- `channel_status` is NOT sent to extensions (it's a Tauri UI WebSocket event only)

---

## 14. Channel Name & Alias Awareness

The daemon maintains an alias system for channel names (see companion spec Section 11). The extension does NOT need to manage aliases - it sends whatever channel name yt-dlp extracts or the user provides, and the daemon resolves aliases transparently. However, the extension should be aware:

- If the extension sends a download for channel "fivee" and the daemon has an alias "fivee" -> "five", the download will be stored under "five". The `download_started` response will reflect the resolved channel name.
- The extension's download queue UI should display the resolved channel name from daemon events, not the originally submitted name.

---

## 15. Daemon Accommodations (compatibility notes)

The daemon has been adjusted to accommodate the extension's actual implementation. These notes document what the daemon accepts and what the extension must fix on its side.

### Daemon-side accommodations (already implemented)

| Item | What the daemon does |
|------|---------------------|
| `auto_start` on `extract_info` | Accepts `auto_start: false` field. When false, skips the 10-minute format selection timeout for that request. |
| `channel_name: null` on `download` | Accepts null/missing `channel_name`. Falls back to URL domain (e.g., `"youtube.com"`). |
| Extra `id` fields on all messages | Silently ignored. Serde does not use `deny_unknown_fields`. Extension can send `id` on any message type. |
| `options` omitted on `download` | Server-side defaults apply: `embed_thumbnail: true`, `embed_metadata: true`. |
| `libraries_installed` boolean in `hello` | Added as a convenience summary: `true` when both yt-dlp and FFmpeg are installed. Extension should still parse the full `libraries` object for version info. |
| `pair_failed` reason strings | Standardized to: `"Code expired"`, `"Invalid code"`, `"Code invalidated"`, `"Rate limited"`. |
| `port_changed` uses `new_port` field | The field name is `new_port` (not `port`). Extension must read `msg.new_port`. |

### Extension-side bugs to fix (in battles-replay repo)

These are known issues in the extension that need to be fixed separately:

1. **Read `msg.version`** not `msg.app_version` from `hello` response - `appVersion` is always null
2. **Parse `msg.libraries` object** instead of expecting `msg.libraries_installed` boolean - although the daemon now sends both, the extension should handle the structured object for version/path info
3. **Read `msg.new_port`** not `msg.port` from `port_changed` - port migration fails silently
4. **Update `librariesInstalled` state** when `library_installed` event is received - currently stays false after install
5. **Add `BRIDGE.*` constants** for `library_download_progress`, `library_installed`, `library_install_failed`, `quota_warning`, `quota_exceeded` events - currently lost silently
6. **Handle unsolicited `error` messages** without matching request `id` - currently falls through unhandled
7. **Fix `bridgeAutoExtract` setting** - toggle exists but auto-extract fires regardless of value
8. **No library install progress UI** - event infrastructure exists but no component renders progress bars (future iteration)
