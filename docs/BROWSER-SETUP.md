# Chrome and Edge setup

## Extension only

1. Extract `WebsiteBlocker-Companion-v0.5.0.zip` into a permanent folder.
2. Open `chrome://extensions` or `edge://extensions`, enable **Developer mode**, choose **Load unpacked** and select the extracted `extension` directory.
3. Use the toolbar popup's **Open focus studio** action. **Websites** manages lists, exceptions and recurring schedules. **Focus** starts sessions and provides deliberate breaks/stopping. **Activity** shows local blocked visits. **Settings** controls language, theme and optional desktop synchronization.

The EXE is unnecessary for every browser workflow above. Native-host unavailability does not show an enforcement failure: local rules continue working. Keep the extension folder in place. An unpacked update means replacing its files and choosing **Reload** on the browser card; preserve the extension identity to retain settings.

The preview is not in Chrome Web Store or Edge Add-ons. Installation/consent cannot be silently completed by the desktop app. Normal operation does not change browser preferences, use enterprise policies, enable Developer mode, use DevTools flags or install an extension behind your back.

## Add the optional desktop app

1. Keep `WebsiteBlocker-v0.5.0.exe` in a permanent folder. Open **Settings → Connect browser**, select Chrome or Edge, and choose **Connect Chrome/Edge**.
2. If the same extension is already installed, keep it. In extension **Settings**, choose **Sync with desktop**. For a new install, the wizard's **Open extensions** and **Copy companion folder** actions guide you through the browser's **Load unpacked** step. Do not enable duplicate copies.
3. If both sides contain independent changes, extension **Settings** shows both list summaries. Choose **Use browser settings** or **Use desktop settings**. The two original copies are retained locally as a sync backup after a choice. Active locked sessions cannot be shortened by choosing another copy.
4. The desktop confirms readiness only after current rules are applied by the extension. The extension synchronizes periodically, normally every 30 seconds while awake; use **Sync with desktop** for an immediate attempt.

Lists, allowlists, schedules, timers, breaks, language and theme travel both ways. Profile-specific attempt counters are mirrored to the desktop and aggregated there. The desktop update preference remains desktop-only. Closing or deleting the EXE does not disable browser blocking or local editing.

## Preparation and storage

The EXE embeds all extension runtime assets. Preparation writes them under `%LOCALAPPDATA%\WebsiteBlocker\browser-companion\extension` and writes a native manifest under `native\Chrome.json` or `native\Edge.json`. It registers only the exact current-user native-host value. No administrator permission is required. Existing foreign/invalid registrations are preserved and reported. The public manifest key stabilizes the preview's extension identity across folders; it is not a credential.

Browser settings/counters remain in that profile's `chrome.storage.local`; desktop settings are in `state.json`. Atomic native writes include revision checks. Unsaved desktop website drafts survive incoming browser edits; stale drafts must be explicitly reloaded before saving. Only configured domains and counts are saved as activity, never full visited URLs or page contents.

## Troubleshooting

- **No EXE:** expected and supported. Use the extension studio; connection status is informational.
- **EXE exists but sync is unavailable:** prepare this browser in desktop Settings first. File presence alone cannot grant Chrome native-messaging permission. If you moved the EXE, choose **Prepare connection again** in the wizard.
- **Independent edits:** use the conflict choices in extension Settings. The popup also points to these settings. No latest-clock-wins rule silently discards a copy.
- **Old v0.4 cache:** connect the updated desktop once to migrate editable source settings. Until then, the old cached policy retains its original timer and expiry; it is never silently replaced by empty lists.
- **Browser rule/schedule error:** correct the draft and save again. Invalid timezone/RE2 rules are rejected before replacing the last usable local settings.
- **Stop enforcement:** deliberately end/pause a flexible session in the studio, edit schedules, or disable/remove the extension. Removing native registration alone does not stop browser rules.
- **Browser sleep:** alarms may run late. An open page is not forcibly closed when its site becomes blocked; navigation/reload triggers enforcement.

Incognito/InPrivate requires separate browser consent. Firefox remains unsupported.

## Developer checks

`node scripts/Browser-Standalone.mjs Chrome` (or `Edge`) checks the real extension in a headless disposable profile: missing-EXE enforcement, real DNR navigations, reload deduplication, editor saving, six languages, both themes, finite/reduced motion, native two-way sync, conflict resolution and lock protection. It temporarily replaces the exact native-host registration with a backed-up test manifest and restores it in `finally`. The data directory, web server and domains are artificial and isolated. Run only one fixture for a given browser registration at a time. `Check.ps1` runs noninteractive unit/structure tests without a browser or registration changes.
