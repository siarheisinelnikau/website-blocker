# WebsiteBlocker

A local, voluntary focus tool for websites. Manage website lists, recurring schedules and timed focus sessions in a Rust desktop app; a Chromium Manifest V3 companion enforces navigation rules in the browser. Independent implementation, with no affiliation to Cold Turkey and no copied code or assets.

**Status: first implementation, Windows x64 target.** Noninteractive checks are recorded in [verification notes](docs/VERIFICATION.md). Real desktop rendering, extension installation, browser enforcement and update/restart behavior still require manual validation. No repository or release has been published in this work.

## What it does

- Up to 20 named website lists with 50 domain or path-prefix patterns each (1,000 patterns total, including allowed websites). Plain domains include subdomains; `=example.com` matches only that host. International domain names normalize to ASCII.
- A global allowlist, taking priority over all active block lists.
- Focus timers from 1 to 240 minutes. Flexible sessions permit deliberate early stopping; locked sessions protect rule edits, breaks and early stopping until the timer ends.
- Recurring weekday schedules in named IANA timezones, with overnight ranges and daylight-saving transitions handled by the browser's `Intl` database.
- Five-minute breaks, at most once per 30 minutes, requiring the phrase `TAKE A BREAK`. Ending a flexible session requires `END FOCUS`; scheduled blocks continue independently.
- A clear browser block page, extension popup with applied rule status and connection errors, and per-user native messaging without a local web server.
- Local JSON settings with validation, atomic replacement, bounded reads, writer locking and stale-edit protection. Browsing history and requested URLs are not stored.
- Settings → Updates shows the installed version, manual **Check for updates**, a persistent startup-check toggle (on by default), and user-initiated **Download and install** with progress, SHA-256 checking and a restart helper. Updates require an official compatible asset and `checksums.txt`; unavailable releases are reported truthfully.

## Try it later on a desktop

1. Build/package locally using the commands below. Keep `WebsiteBlocker-v0.1.0.exe` in a stable, user-writable folder. Extract the companion ZIP into another stable folder.
2. Follow [browser setup](docs/BROWSER-SETUP.md) to load the extension and explicitly register its native host for Chrome or Edge. This step changes one per-user registry entry and is not performed by the app or build.
3. Open the EXE. Edit **Websites & schedules**, save, and use the extension popup to confirm a successful sync. The initial example list is timer-only and does not block anything until a session starts or a schedule is saved.
4. Start a short flexible session on a test domain first. Navigation matching requires the extension to be enabled in that browser profile.

Example pattern list:

```text
example.com
=video.example.org/shorts
news.example.net/feed
```

`/feed` is a literal, case-sensitive prefix: it also matches `/feedback`. Schemes, ports, query/fragment patterns, wildcards in paths, IP addresses and single-label intranet hosts are intentionally unsupported in v0.1. Rules apply to HTTP(S) top-level navigations, including navigations on nondefault ports.

Closing the desktop app leaves browser schedules active. The extension refreshes native state about every 30 seconds while awake and also schedules boundary alarms. Browser throttling/sleep can delay transitions; a 30-second refresh is not a hard real-time guarantee. On connection failure, the last usable policy continues locally for up to eight days, with its original focus end time. After expiry or a detected large clock rollback, blocking clears and the popup reports an error. A user who changes rules while disconnected must reconnect the extension before those edits take effect.

## Build and check

Use Rust 1.92+ with the Windows MSVC toolchain, its existing C++ build tools, and Node 22+ for dependency-free extension tests. Build commands never launch the GUI, install the extension, register a host or run the updater.

```powershell
Set-Location E:\work\github\website-blocker
cargo fetch --locked                       # only when dependencies are not already cached
.\scripts\Check.ps1                        # offline formatting, Clippy and isolated tests
.\scripts\Package.ps1 -Offline              # Windows x64 EXE, companion ZIP, checksums
python scripts/Inspect-Package.py           # inspect PE/ZIP/hashes without execution
```

Cargo uses a repository-local `target` directory, at most two build jobs and the static MSVC CRT for Windows x64. The workflow prepares build artifacts without publishing releases. First dependency fetch requires network access; the implementation environment used existing cached crates.

Expected local artifacts:

```text
dist/WebsiteBlocker-v0.1.0.exe
dist/WebsiteBlocker-Companion-v0.1.0.zip
dist/checksums.txt
```

The single executable contains both the desktop app and its native messaging host. Browser extension files and the explicit registration script are a necessary exception to an EXE-only distribution. Source is also portable in parts, but only Windows x64 packaging is currently provided. Firefox is a future port, not a supported browser in this release.

## Limits and privacy

This is not a security boundary or an uninstall-proof lock. The machine owner can disable/remove the extension, change local settings or the clock, use a different profile/browser or edit the source. It does not change hosts, DNS, firewall settings, processes, services or devices. No application/process blocking is included.

Already loaded pages keep running until a navigation/reload; single-page-app history changes, embedded content, downloads, internal browser pages and other applications are outside this version's enforcement scope. The extension requests HTTP(S) host access to redirect selected navigations, plus declarative rules, local storage, alarms and native messaging permissions. Incognito/InPrivate requires separate consent in the browser.

Settings live at `%LOCALAPPDATA%\WebsiteBlocker\state.json`. Keep a copy before manually editing. Corrupt or unsupported files are preserved and reported. Updater downloads and result logs live under `updates/`; personal settings are kept outside the EXE directory and are not replaced during updates. Update checks contact GitHub only; browser policies stay local. SHA-256 detects incomplete or corrupted assets but is not an independent publisher signature. Builds are currently unsigned.

## Screenshots

Real screenshots of the application and Settings are **pending**. This implementation session explicitly prohibited launching GUI applications or taking screenshots. No mock screenshots or broken image placeholders are supplied. Capture and embed real images after the manual validation checklist passes.

See [architecture](docs/ARCHITECTURE.md), [manual checks and verification](docs/VERIFICATION.md), and the [first release draft](docs/RELEASE-NOTES-v0.1.0.md). MIT-licensed; third-party dependencies retain their own licenses as declared in their pinned crate distributions.
