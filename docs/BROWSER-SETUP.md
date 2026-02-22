# Connect Chrome or Edge from WebsiteBlocker

The EXE contains the companion. Use the connection wizard; no terminal commands, extension-ID copying, manual native manifest or registry editing are required.

1. Keep the EXE in a permanent folder. Open **Settings → Connect browser**, or the **Connect browser to start** action on Focus.
2. Select **Chrome** or **Edge**, then **Connect Chrome/Edge**. WebsiteBlocker extracts its included companion into its own data directory and registers its native host for the current Windows user. No administrator access is requested.
3. Select **Open Chrome/Edge extensions**, then **Copy companion folder** in the wizard. In the browser's extension page, enable **Developer mode**, choose **Load unpacked**, and paste/select that copied directory in the folder picker.
4. The companion opens its welcome page and applies the current rules. The desktop checks for confirmation automatically while setup is open. If needed, use **Check connection** on the companion welcome page, then **Check connection** in the desktop wizard.
5. When the wizard confirms the connection, select **Go to Focus**. Repeat in other browser profiles where blocking is wanted. A connection in one profile does not enable the companion in every profile.

## Why one browser step remains in this preview

The extension has not been published in Chrome Web Store or Edge Add-ons. On Windows, ordinary Chrome external installation must come from the Chrome Web Store and the user must consent in the browser. The preview therefore uses the browser's supported unpacked-extension flow. The app does not change browser preferences, install by enterprise policy, use unsafe debugging flags in normal operation or silently enable extensions. A normal store installation flow requires a real published store listing; no fake listing or unavailable Install button is provided.

Primary references: [Chrome extension distribution](https://developer.chrome.com/docs/extensions/how-to/distribute/install-extensions), [Edge extension distribution](https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/alternate-distribution-options), [native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging), [stable development identity](https://developer.chrome.com/docs/extensions/reference/manifest/key).

## What the app prepares

Companion files are kept in `%LOCALAPPDATA%\WebsiteBlocker\browser-companion\extension`. The public preview manifest key keeps its extension ID stable across extraction paths. This public key is not a credential; store publication must use the eventual store identity and migrate allowed origins deliberately.

Native manifests live under `%LOCALAPPDATA%\WebsiteBlocker\native\Chrome.json` or `Edge.json`. Only the exact per-user native-host default value is registered, under Google\Chrome or Microsoft\Edge. Other browser settings/profiles are untouched. Preparation is repeatable, preserves rules/focus settings and retains valid existing origins from this app's own manifests for migration. A foreign or invalid registration is preserved and reported rather than overwritten blindly.

The connection label is based on a recent, version-matched applied-rule report from the browser, not merely a written registry value or native snapshot request. A connected profile can show that recent rule changes are still syncing. An application error or missing/stale report is not shown as current-rule readiness. Reports contain revision/version/status, not browsing history or visited URLs.

## Troubleshooting

- **Old preview already added:** disable/remove that earlier WebsiteBlocker copy before loading the managed companion folder. Keep one enabled copy per profile. Older path-derived IDs differ from the new fixed preview identity.
- **Files prepared but not connected:** finish the browser step, ensure the companion is enabled in the intended profile, and use its welcome-page Check connection action. Native sync normally occurs within about 30 seconds while the browser is awake.
- **Already loaded the managed companion:** reload its card in the browser after preparing updated files. Local unpacked-extension updates still require that browser action; store-managed updates are not available yet.
- **EXE moved:** run the current copy, open the wizard and choose **Prepare connection again** under repair. This updates the native path without editing files.
- **Browser closed:** open it; its startup sync will renew the connection report. Cached schedules remain local to profiles with the extension enabled.
- **Unknown timezone or rule error:** correct the schedule/list in WebsiteBlocker and sync again. The last usable cached policy continues until its expiry; the UI reports the failure.
- **Stop browser enforcement:** disable/remove the companion in that browser profile. Removing only native registration would leave a valid cached policy until it expires, so the wizard does not present that as an immediate Stop action.

Settings and rule files are outside the EXE directory and survive connection preparation. Private/Incognito mode requires separate browser consent. This is voluntary blocking, not a security boundary.

## Developer verification

The production setup backend is tested with a fake registry and isolated files. `examples/prepare-connection.rs` exercises the same backend headlessly for opt-in live fixtures. After building that example, `node scripts/Browser-Smoke.mjs Chrome --headless --managed` (or Edge) tests the embedded extraction, real per-user Rust registration, applied-rule status and navigation enforcement. It uses disposable profiles, backs up/restores the exact native-host value and never uses regular profiles or credentials. These opt-in live tests are separate from Check.ps1 and CI. Legacy PowerShell setup scripts remain developer/backward-compatibility tools; the consumer workflow does not require them.

Firefox remains unsupported: it needs a reviewed manifest/API adaptation, Mozilla native-host registration and tested rule semantics.
