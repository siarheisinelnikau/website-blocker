# WebsiteBlocker v0.3.0 — local preview, unpublished

This preview improves the desktop and blocked-page experience and fixes missing application icons and folder copying. Changes follow the local v0.2.0 milestone; there is no preceding published GitHub release.

## Added

- A custom offline block page with the active website-list names, a forest-green orbital composition, pointer parallax and a deliberate 12-second breathing exercise. Entrance animation is finite, parallax/breathing clear when hidden, and physical movement respects reduced-motion preferences.
- A shared focus-target icon in nine native Windows sizes and four browser-companion sizes. The EXE contains actual icon and version resources; the window and taskbar/Alt-Tab icons use the embedded resource at the window's DPI.
- Package checks reject missing/mismatched icon resources, file/product versions and companion PNG references. A read-only native proof renders the shipped resource and compares the running window's assigned icons against it.

## Improved

- Reduced the initial desktop client area from 880 × 680 to 740 × 520, retaining readable text and control sizes. Website list and duration sit side by side, Settings shows its version in the heading row, and repeated guidance is removed.
- Browser setup reveals installation steps after preparation instead of showing a full page of unavailable actions.

## Fixed

- Copy companion folder now returns a verified existing directory with native Windows separators, without a verbatim path prefix, preserving spaces and Unicode. The wizard shows the path and explains how to paste it into the folder picker's address bar.
- Missing manifest files now produce an error instead of claiming a usable folder was copied. Preparation and path validation stay on the settings worker.
- Old companion manifests are recognized as needing preparation; prepared connections also expose Update connection files for repair and upgrades.

## Upgrade and limitations

Windows x64 asset: `WebsiteBlocker-v0.3.0.exe`, optional `WebsiteBlocker-Companion-v0.3.0.zip`, and `checksums.txt`. Run the new EXE from a permanent folder, prepare its browser connection again, then reload the managed companion's card in Chrome/Edge to pick up the new page and icons. Existing settings remain outside the EXE directory.

Builds remain unsigned and unpublished. Developer mode/Load unpacked is still required for this local companion. Native file-picker interaction, ordinary taskbar/Explorer rendering, mixed-DPI moves, perceived OS-input smoothness and real update installation are not certified by client-area PNGs or resource proofs. See [verification notes](VERIFICATION.md) for the measured scope.
