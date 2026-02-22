# WebsiteBlocker v0.1.0 — draft, unpublished

First implementation of a local website focus manager with a Chromium companion. There is no preceding published release or remote comparison baseline.

## Added

- Named domain/subdomain and literal path-prefix lists, global allowed websites and validated local settings.
- Timed flexible and locked focus sessions, deliberate early stopping and cooldown-limited five-minute breaks.
- Weekday/overnight schedules in named timezones, with daylight-saving evaluation in the browser.
- A Chromium Manifest V3 navigation redirect engine, clear block page, local cache and connection/status popup.
- An EXE-based native messaging host and explicit per-user setup/removal scripts for Chrome and Edge.
- Background startup update checks with an opt-out, manual checks, verified download progress and a portable replacement/restart helper for future compatible GitHub releases.
- Offline Rust/JavaScript fixture tests, Windows checks and local packaging scripts.

## Browser connection

- Added Settings → Connect browser with Chrome/Edge selection, automatic embedded-companion extraction and per-user native registration. No consumer PowerShell or manual extension-ID/configuration entry is required.
- Added stable preview identity, welcome-page sync and verification of actually applied browser rules. Focus setup routes unconnected users to the wizard.
- Verified the same setup backend and managed embedded extension in headless Chrome/Edge fixtures. The unpublished preview still requires the browser's Developer mode/Load unpacked step; a store listing is not published.

## Improved

- Replaced the desktop OpenGL renderer with CPU rasterization and Win32 GDI, and stopped periodic rendering of unchanged or hidden windows. Bounded diagnostics check actual frame counters and process CPU.
- Simplified focus setup, increased typography/control sizes, and moved less frequent help/setup details into expandable sections.
- Preserved unsaved website drafts across update-setting and focus actions.
- Added real application and Settings captures plus opt-in rendering and Chrome/Edge integration checks.

## Downloads and requirements

Planned Windows x64 standalone asset: `WebsiteBlocker-v0.1.0.exe`. Also provide `WebsiteBlocker-Companion-v0.1.0.zip` and `checksums.txt`. The EXE includes the companion and its connection wizard; the preview browser installation/consent step remains explicit. Builds are unsigned. No installer, extension store listing or release is published yet.

## Known limitations

Real CPU-rendered screens and isolated Chrome/Edge native communication/enforcement have been checked. OS keyboard/mouse behavior and subjective smoothness after the renderer replacement, high-DPI/private-mode behavior, and real update/restart behavior remain unverified. Rules affect new top-level HTTP(S) navigations/reloads, not already running pages or SPA history changes. Browser alarms may delay transitions, particularly after sleep. This is voluntary blocking which the machine owner can bypass; no process, DNS or system-wide blocking is provided. Firefox support is pending. Read docs/VERIFICATION.md before treating this draft as a validated release.
