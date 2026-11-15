# WebsiteBlocker 0.5.0

Compared with v0.4.0, the browser extension is now a standalone focus manager. The Windows app is optional and adds synchronized desktop management.

## Added

- Browser Focus Studio with editable lists, global exceptions, recurring schedules, focus timers, deliberate breaks/stopping, language and theme controls.
- Two-way desktop synchronization with revision checks, explicit independent-edit conflict choices and retained local copies. Locked sessions remain protected. Existing native-host registration still requires the user's desktop Connect action.
- Real blocked-visit counters, today's total, top configured domains and recent attempts. Per-profile summaries synchronize into the EXE without recounting repeated syncs. Full visited URLs and page contents are never stored.
- A dimensional focus-shield scene with intercepted website tiles, impact rings, particles, checkmark drawing and replay. Motion stops after its sequence or when hidden and honors reduced-motion settings.
- Desktop section arrivals, spring shield motion and two-way connection packets, plus an explicit scene replay action. Software-rendered frames stop after transitions finish.

## Improved

- Local browser schedules no longer depend on renewing an eight-day desktop cache. Removing or closing the EXE leaves local rules and editing available.
- Incoming browser edits refresh the desktop; unsaved website drafts survive and cannot silently overwrite a newer revision.
- The toolbar popup starts a focus session and opens the full browser manager. Missing native messaging is an optional sync state rather than a blocking error.
- Extension and desktop remain version-matched at 0.5.0, with six languages and light/dark themes.

## Upgrade and limits

Unpacked extensions need their files replaced and their browser card reloaded. Keep the stable extension identity. Earlier compiled policy caches require one connection to the updated desktop to recover editable source settings; their original cache lifetime is preserved until migration. For new installations, only the companion ZIP is needed. Native setup, if wanted, remains explicit.

Windows x64 EXE: `WebsiteBlocker-v0.5.0.exe`. Standalone extension bundle: `WebsiteBlocker-Companion-v0.5.0.zip`. The preview is unsigned and unpublished in browser stores. Local test evidence is in [verification](VERIFICATION.md); OS input/accessibility, subjective smoothness and real update replacement remain manual checks. No remote publication is performed by this change.
