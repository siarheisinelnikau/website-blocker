# Architecture and behavior

## Data flow

```text
Desktop UI -> bounded settings worker -> validated state.json (atomic replacement)
Browser alarm / Sync -> EXE native host -> read-only policy snapshot
Snapshot -> local extension cache -> Intl schedule evaluation -> Chromium dynamic rules
Applied-rule report -> per-browser connection status -> in-app setup confirmation
HTTP(S) top-level navigation -> higher-priority allow OR redirect -> local block page
```

The desktop uses egui with a consistent green/neutral visual system, readable text and larger controls. Focus setup is the primary task; the website editor and settings have separate routes. Matching help, weekly schedules, exceptions, browser installation steps and local-data details expand only when needed. The desktop remains an editor/session manager rather than a tray-resident monitor: closing it exits, while the browser owns enforcement.

Settings I/O and updates use separate bounded worker queues. A slow update does not block editing or starting focus. A settings change or focus action preserves unsaved website drafts; only explicit reload/discard or successful rule saving rebuilds the editor. Progress messages are throttled and bounded.

## Software rendering and scheduling

The executable has no eframe, Glow, wgpu, OpenGL or DirectX rendering path. The verified route is `egui -> egui_software_backend 0.0.2 (CPU) -> retained BGRA bitmap -> softbuffer 0.4.8 Win32 GDI BitBlt`. The pinned softbuffer Win32 source was inspected. PE imports and the running diagnostic's selected loaded-module checks support this path; Windows desktop composition remains an OS responsibility.

A winit event loop uses Wait/WaitUntil and no Poll. Input, changed worker data, resize/DPI and a visible focus countdown can invalidate the scene. Unchanged OS expose events can blit the retained bitmap without layout/rasterization. A static Focus/Websites page has no periodic UI timer. Settings can request a background connection read every 30 seconds; unchanged reads do not publish a repaint. A visible countdown repaints once per second. Hidden/minimized windows perform no layout, rasterization or presentation; dirty state is retained for restoration. Workers can complete and request shutdown while hidden without requiring a painted frame.

The Win32 winit 0.30 loop may wait immediately after AboutToWait, before observing an exit requested from that callback. The host expires that one final wait and releases the GDI surface/window during exiting. Diagnostic and capture modes have internal limits and independent external timeouts. They are not consumer settings.

`WEBSITEBLOCKER_DATA_DIR` selects an explicit absolute isolated data directory for the desktop and native host. `WEBSITEBLOCKER_CAPTURE=focus|ABSOLUTE.png` (or rules/settings/browser) selects a real route, waits for settings/browser-status readiness and three bounded UI passes, atomically writes the CPU bitmap as PNG and exits. Unknown routes, invalid output paths and failures return nonzero. Capture/diagnostic modes require an explicit data directory and suppress startup update checks. `WEBSITEBLOCKER_DIAGNOSTICS` writes opt-in aggregate counters, phase CPU samples and targeted module observations during a bounded visible/minimized/restored/hidden test. No per-frame production logging or automation server is included.

## Rules and schedules

Rust validates list size/IDs, syntax, canonical domain/path representation, session policy and persistence. The extension validates snapshot structure, actual IANA zone support, and Chromium RE2 compatibility before replacing its cached policy. The browser owns timezone evaluation, using `Intl.DateTimeFormat` with a named zone and 24-hour clock. No new timezone database dependency or network lookup is required. Rust `Schedule::active_wall` and the extension implement the same half-open wall-minute predicate. Timezone conversion and daylight-saving fixtures are exercised in Node.

Weekly days use Monday = 0. Start is inclusive, end is exclusive. Equal start/end is invalid; add separate nonoverlapping schedules for longer ranges. Overnight schedules are attributed to their start day. At fall-back, repeated matching local minutes block twice; at spring-forward, nonexistent local minutes do not occur. A focus session OR any active schedule activates a list. A global pause overrides both. Disabled lists produce no policy groups.

Focus and pause timestamps use UTC seconds, so their intended durations do not change across DST. The extension evaluates on startup, native sync, and boundary alarms. Chrome's alarms are not exact timers and may be delayed after sleep; applied DNR rules can therefore outlive the logical end until the next wake. Continuous media and already loaded pages are not forcibly interrupted. Clock changes can alter schedules/timers; a rollback more than 60 seconds before the cached policy generation time clears blocking until a fresh policy arrives. This is a voluntary tool and not resistant to deliberate clock manipulation.

All allow patterns have priority 100 and all redirects priority 1. Allowlists are deliberately global; they do not silently override only one selected list. DNR conditions are case-sensitive regexes anchored at HTTP(S) and host boundaries, with canonical lowercase/punycode domains and literal URL path prefixes. At most 1,000 patterns are accepted across all lists and the allowlist, respecting Chromium's regex-rule limit; individual RE2 memory/complexity limits are checked with `isRegexSupported`. Rule replacement is one browser API operation, with unique IDs and removal of previous IDs. On API rejection, the popup reports an apply error; it does not claim enforcement succeeded.

## Persistence and protocol

All runtime data is under the user's WebsiteBlocker directory. State files are bounded to 1 MiB, validated on read, and never overwritten on parse/schema failure. A separate file lock serializes reads/writes; acquisition fails promptly rather than blocking a UI worker forever. Rule saves include their base revision to reject stale editors. Writes use a same-directory temporary file, flush and atomic replacement. Empty initial settings are loaded as an inactive timer-only example list. Tests use temporary fixture directories under `.test-output/`; real `%LOCALAPPDATA%` is never touched by tests.

Native messaging uses native-endian 32-bit framing, bounded to 4 KiB input and less than 1 MiB output. Protocol 1 accepts the legacy snapshot command plus a strictly typed applied-status report (browser, revision, fixed extension ID, version, bounded error). Unknown fields/commands and browser-side attempts to stop focus remain rejected. Applied reports cannot modify rule/focus configuration. They are written only after the extension successfully updates DNR, or carry an explicit error. Current-rule confirmation requires a recent report with the saved revision; transport connection and pending rule synchronization are separate UI states. This is a cooperative local status mechanism, not tamper-resistant attestation.

## In-app browser setup

The Settings/Focus action opens one coherent connection wizard. The EXE embeds a fixed list of companion files using include_bytes; no network download or external extraction utility is involved. An explicit Connect action writes only managed local files and the exact current-user native-host default value through Win32 registry APIs. It never runs PowerShell, changes browser preferences/policies, enables Developer mode or loads an extension silently. File writes are atomic and repeatable. Existing valid origins in app-owned manifests are retained; foreign registrations are preserved with a clear error. A manifest public key makes preview IDs stable and removes manual ID entry.

A bounded worker handles preparation, status inspection and explicit browser launching; no disk/registry work is performed by the paint callback. While setup waits, it reads status about once a second for at most two minutes; stable status/errors do not publish repaint events. Otherwise inspection is every 30 seconds while visible. Hidden windows still perform no UI rendering. Diagnostic/capture fixtures ignore real keyboard/mouse input to prevent accidental connection actions, and count ignored input separately. Normal mode continues processing input; it skips raster/present when shapes, dimensions, scale and texture deltas show the scene is unchanged. Native expose/restore still blits the retained bitmap.

The companion's install welcome page initiates a sync and displays connection success/failure. Chrome/Edge browser consent and unpublished-extension loading remain explicit browser steps. A future store path needs a real published listing and identity; none is invented. Production setup uses no DevTools flags; those exist only in the isolated developer smoke driver.

The legacy global handshake file remains for protocol compatibility. Per-browser applied records are stored under connections/. Native requests are short-lived and have no GUI. The extension stores a snapshot and compact status, with an eight-day maximum cache lifetime. Focus end times do not extend on disconnect. Expired policies clear browser rules and report an error.

## Updates and distribution

Update checking uses the public latest-release endpoint of `siarheisinelnikau/website-blocker`, no credentials or Codex connector. Drafts/prereleases and non-newer semantic versions are excluded. The only supported install target is Windows x64 with `WebsiteBlocker-vVERSION.exe` and `checksums.txt`. URLs must point to release assets in the canonical repository. Missing compatible assets, network errors, missing checksums and invalid bodies are errors, not up-to-date results. Startup checks honor the saved toggle before making a request and report quietly inside Settings. Only an available-update banner appears outside Settings.

Download/install starts only after the user's explicit button click. Downloads stream to a temporary file with a 200 MiB cap, timeout, length check, SHA-256 and PE `MZ` header check. A copy of the current EXE acts as a helper, waits for the UI process to exit, copies the staged asset to a temporary file on the installation volume, preserves the old EXE, replaces it, and requests relaunch. Staging/replacement/relaunch failures preserve or restore the old EXE where possible and write `updates/install-result.txt`. A successful process spawn is not proof the new GUI booted; no post-launch health handshake is implemented. There is no code-signature validation or unattended install. A protected installation directory requires a manual update to a writable folder; elevation is never requested.

Settings remain outside the install directory. Helper copies can remain in `updates/` because Windows cannot delete the running helper itself; remove old `apply-*.exe` files after a successful update when no helper is running. Companion extension updates are separate and manual in this first version; desktop protocol version 1 is required. A future incompatible extension protocol must require a coordinated companion update.
