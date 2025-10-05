# WebsiteBlocker

Repository: `website-blocker`
Priority: 9 of 15 (user-requested launch order)
Product: WebsiteBlocker
Purpose: Website-focused distraction blocking
Inspiration: https://getcoldturkey.com/

Independent open-source implementation; no affiliation, competitor code or copied assets.

## Requested implementation

The user explicitly prioritizes websites, not processes. Build a desktop schedule/rule manager and the browser-side component needed to block selected domains, subdomains and optionally URL patterns, with allowlists, timed focus sessions, recurring schedules, clear block pages and an intentional unlock/break policy. Use a practical Chromium WebExtension first and structure it for Firefox compatibility where feasible. Keep browser UI and settings understandable to ordinary users.

A small extension/native-messaging companion may be necessary; document this honest exception to the single-EXE preference. Prefer local state and explicit browser consent. Do not claim blocking is impossible for the machine owner to bypass. Do not change hosts/DNS/firewall settings, install extensions, manipulate real browsers, start focus sessions on this PC or lock the user out. Implement real matching/scheduling and extension integration source, test with fixtures including clock/timezone boundaries, and leave manual extension installation for later.

## Packaging and publication target

Windows standalone asset: `WebsiteBlocker-vVERSION.exe` (for the initial release, `WebsiteBlocker-v0.1.0.exe`). Append architecture only when distributing multiple executable architectures in the same release.
Planned canonical repository: https://github.com/siarheisinelnikau/website-blocker (not yet created or published; keep availability/error reporting truthful).

## Working agreement

Follow AGENTS.md. Complete as much real implementation and safe noninteractive verification as possible in this unattended code-only session. Manual UI, device and hardware tests will happen later. Record required external components and their verification status. Keep changes committed locally in meaningful increments.

