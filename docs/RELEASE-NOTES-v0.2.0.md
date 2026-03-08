# WebsiteBlocker v0.2.0 — local preview, unpublished

This version gives the browser-connection milestone a distinct number. Earlier development builds, including the initial wizard, were incorrectly handed over under the same v0.1.0 label. No GitHub release or extension-store listing has been published, so there is no preceding published release to compare against.

## Browser connection milestone

- Settings → Connect browser prepares the embedded Chrome/Edge companion and per-user native registration without PowerShell, extension-ID entry or configuration-file editing.
- The wizard opens the browser's extensions page, copies the managed companion folder and confirms that current rules were actually applied. Focus directs unconnected users to setup.
- CPU rasterization and Win32 GDI replace the original OpenGL renderer. Unchanged scenes skip rasterization; hidden windows stop UI rendering and restore uses the retained frame.
- Real application, Settings and connection-wizard screenshots are included in README. Isolated tests cover registration, native communication and navigation enforcement in both browsers.

## Versioning correction

- The desktop package, embedded companion, installed-version display and artifact names now agree on 0.2.0.
- Browser smoke, renderer diagnostics and screenshot scripts derive the current version instead of selecting an old EXE by a hardcoded name.
- Future changed distributables must receive a new version before being handed over. Individual source commits do not each require a version bump.

## Upgrade from a local v0.1.0 build

Keep the new EXE in a permanent folder. In Settings → Connect browser, prepare the connection again to update the native-host path and managed companion files. If the managed extension is already loaded, reload its card in the browser's extension page. Settings and website lists stay in the existing local-data directory. If an older path-derived copy is installed, remove/disable it and load the managed folder once.

## Downloads and limitations

Windows x64: `WebsiteBlocker-v0.2.0.exe`, optional `WebsiteBlocker-Companion-v0.2.0.zip`, and `checksums.txt`. Builds are unsigned. The unpublished extension still requires Developer mode → Load unpacked and explicit browser consent. Firefox is unsupported. Normal OS-input smoothness, high-DPI/private-mode behavior and real update installation remain unverified; see [verification notes](VERIFICATION.md).
