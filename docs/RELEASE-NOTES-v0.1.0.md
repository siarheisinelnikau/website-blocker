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

## Downloads and requirements

Planned Windows x64 standalone asset: `WebsiteBlocker-v0.1.0.exe`. Also provide `WebsiteBlocker-Companion-v0.1.0.zip` and `checksums.txt`. The EXE and unpacked extension require explicit manual connection setup. Builds are unsigned. No installer, extension store listing or release is published yet.

## Known limitations

GUI rendering, real Chrome/Edge native communication and enforcement, private-mode behavior, and real update/restart behavior remain manually unverified. Rules affect new top-level HTTP(S) navigations/reloads, not already running pages or SPA history changes. Browser alarms may delay transitions, particularly after sleep. This is voluntary blocking which the machine owner can bypass; no process, DNS or system-wide blocking is provided. Firefox support and real README screenshots are pending. Read docs/VERIFICATION.md before treating this draft as a validated release.
