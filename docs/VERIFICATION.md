# Verification record

Implementation date: 2026-10-31 (Europe/Minsk). Work was explicitly restricted to code, compilation and safe noninteractive fixtures. No GUI application was launched, browser/registry/user settings were changed, extension installed, focus session started on the real machine, remote Git operation performed or release published.

## Automated checks

- Rust tests: domain/path boundaries and international domains; wall-minute overnight/week boundaries; precise focus/break policy transfer; persistent locks/break cooldown; stale revision and corrupt-file preservation; native framing/size limits and rejection of mutation commands; semantic release/asset/checksum selection; staged file replacement on isolated data; time-input bounds.
- Node tests (no browser or network): named-zone midnight/week conversion; fall-back and spring-forward fixtures; focus/break/cache expiry and rollback boundaries; DNR allow/redirect payloads and host/path boundaries; invalid policy rejection; service-worker behavior using a fake Chrome API, including native disconnection and rule removal at timer expiry.
- Formatting/Clippy, release compilation, packaging and PowerShell parse results are finalized in the local completion report. CI is prepared but has not run remotely.

Initial download of the optional `chrono-tz` dependency was unavailable (`curl failed: getaddrinfo() thread failed to start`). That dependency was removed; the implementation uses the browser/Node `Intl` timezone database and resolves its Rust dependencies offline. There is no remaining timezone download requirement. Initial replacement tests in the OS temporary directory encountered sandbox access denial; all filesystem fixture tests now use repository-local `.test-output/` and pass there. No elevation was requested.

## Manual validation before release

- [ ] Launch the packaged EXE on supported Windows x64; verify rendering, keyboard navigation, scrolling, clipping, small window sizes and 100/150/200% DPI.
- [ ] Capture real Focus/Rules and Settings screenshots; embed them in README with accurate English captions.
- [ ] Measure visible idle/countdown, minimized and active-interaction CPU; check for frame/input stalls with Glow/OpenGL. The event-loop adapter is compiled, not live-performance verified.
- [ ] Load the unpacked extension in Chrome and Edge; register each separately; confirm the native host writes framed responses without opening a window, including when the desktop is closed.
- [ ] Confirm the popup distinguishes applied state, native handshake, cache mode and errors. Move the EXE temporarily and verify connection-failure/recovery behavior using disposable settings.
- [ ] Navigate/reload a harmless test domain, its subdomain, an unrelated suffix domain, an allowed path, a case-different path and a nondefault port. Verify global allow precedence across two active lists and a readable local block page.
- [ ] Verify no enforcement in another profile/browser without the extension; separately consent to and test Incognito/InPrivate if needed.
- [ ] Verify a flexible timer ends, a locked session rejects editing/break/stop, a typed break pauses all groups for five minutes, its cooldown persists after restart, and ending focus does not cancel an independent schedule.
- [ ] Use disposable settings for schedule start/end, overnight/week rollover, multiple schedules and both DST transitions. Check timezone errors in the popup, sleep/wake and browser restart behavior. Record observed alarm delays.
- [ ] Check that existing pages, embedded content and SPA history behavior match the documented limits.
- [ ] Verify corrupt/unsupported state is preserved and the UI offers a load retry without overwriting the file. Back up only disposable test data.
- [ ] Against a controlled unpublished test release/fixture server setup, validate rate limits, missing release, no-update, newer-version, bad hash, missing asset, interrupted download, read-only install directory, replacement failure, restart failure, successful install and preservation of settings. Do not alter historical public releases for a test.
- [ ] Inspect native registration -WhatIf output before user-authorized setup/removal; confirm unrelated registry keys are untouched.
- [ ] Confirm extension and EXE update compatibility; update the unpacked extension manually when required. Test uninstall and stale-policy expiration.
- [ ] Review pinned dependency licenses/notices, signed-build plans, canonical repository availability, release artifacts and checksums before publication.

The tests prove their isolated logic and payloads only. They do not establish real browser enforcement, GUI usability, timing guarantees or successful installation on a desktop.
