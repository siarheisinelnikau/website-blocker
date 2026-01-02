# Browser setup (manual, Windows)

Chrome 156.0.8078.48 and Edge 156.0.4314.50 passed isolated live native-messaging and navigation checks. The test-only registration was restored afterward. The following steps are still required to enable the extension in your normal browser profile. Chromium 120+ is the declared minimum; older versions have not been tested.

1. Place the EXE in a stable user-writable directory and extract the companion ZIP to a stable directory. Do not keep an unpacked extension in a temporary download folder.
2. Open your browser's extension management page (`chrome://extensions` or `edge://extensions`), enable Developer mode, choose **Load unpacked**, and select the `extension` directory. Review the requested permissions. Copy its 32-character extension ID.
3. Open PowerShell in the extracted companion directory. Run the script with the real absolute EXE path and copied extension ID:

   ```powershell
   .\scripts\Register-NativeHost.ps1 -ExePath 'C:\Tools\WebsiteBlocker-v0.1.0.exe' -ExtensionId 'abcdefghijklmnopabcdefghijklmnop' -Browser Chrome -WhatIf
   # Review the paths, then run the same command without -WhatIf to register.
   ```

   The shown ID is an example, not the real extension ID. The script writes `%LOCALAPPDATA%\WebsiteBlocker\native\Chrome.json` and the default value of `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.websiteblocker.companion`. For Edge use `-Browser Edge`; its registry location is under `Microsoft\Edge`. Only the given extension origin is permitted. A browser hosts one configured extension ID per registration. No administrator rights or service installation are needed. If local execution policy blocks the script, inspect the script and use your organization's approved script execution process; do not disable system-wide protections.
4. Click the extension icon and **Sync with desktop**. The browser launches the same EXE with its extension origin as an argument. This enters a short-lived native messaging path and does not create a desktop window. The desktop does not need to stay open.
5. In the app, make a short flexible test session blocking `example.com`, with `=www.example.com` as an optional allowlist experiment. Navigate or reload to exercise the rule, then intentionally end the test. See the full manual checklist before relying on locked focus.

If the popup says the host was not found, verify the registry value, executable path, extension ID and browser choice. Restarting the browser may be necessary after registration. If the EXE moves or an unpacked extension gets a new ID, rerun registration explicitly. In-app updates replace the EXE at its existing path so the registration remains valid. The installed filename may retain the prior download version after an in-place update; Settings shows the actual installed version.

Check the popup, not only the desktop handshake: the latter records a native request before Chromium applies its rules. A malformed timezone is rejected by `Intl` in the extension; the popup reports it and retains the last usable policy rather than silently accepting a partial ruleset. Correct the timezone and sync again. No successful sync within eight days causes the cache to expire and rules to clear.

To disconnect deliberately:

```powershell
.\scripts\Unregister-NativeHost.ps1 -Browser Chrome -WhatIf
# Review, then omit -WhatIf. Also disable/remove the extension in the browser.
```

Unregistering alone leaves the extension's last policy active until its cache expires; disable/remove the extension to stop browser enforcement immediately. Unregistering preserves personal settings. Removing an enabled extension clears its browser rules.

## Firefox port notes

The pure `engine.js` and static pages do not depend on Chromium. The native message framing is common to WebExtensions. A Firefox port still needs a reviewed manifest/background configuration, `browser` API adaptation, `allowed_extensions` native manifest and Mozilla registration location, stable extension ID, and tested declarative redirect/allow priority semantics. This version intentionally does not ship an unverified Firefox manifest.

Primary API references used during implementation: [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging), [declarativeNetRequest](https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest), [alarms](https://developer.chrome.com/docs/extensions/reference/api/alarms).

## Opt-in live developer test

`node scripts/Browser-Smoke.mjs Chrome` or `Edge` launches a disposable profile under `.test-output/`, serves harmless HTTP fixtures on loopback, temporarily points the per-user native-host registration to the test manifest, then restores its original default value. It never uses the normal browser profile or credentials. Use `WEBSITEBLOCKER_TEST_EXE` to select an absolute built EXE; the default is `dist/WebsiteBlocker-v0.1.0.exe`. The test intentionally launches a browser and changes a temporary native registration, so it is separate from Check.ps1 and is never run implicitly by packaging or CI. Chrome uses a DevTools pipe; Edge's launcher required a local DevTools port. These debugging flags apply only to disposable test profiles and do not disable the browser sandbox.
