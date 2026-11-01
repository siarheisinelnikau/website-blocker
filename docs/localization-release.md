# WebsiteBlocker v0.4.0

This is the first published release of WebsiteBlocker.

## Initial capabilities

- Manage named website lists, allowlisted sites, flexible/locked focus timers and recurring timezone-aware schedules.
- Connect the embedded Chromium companion through an explicit browser-consent workflow and show local rule/connection status and an offline block page.
- Keep policies local without editing hosts, DNS or firewall settings. The companion still requires its documented browser installation step and is not an uninstall-proof security boundary.

## Localization and appearance

WebsiteBlocker now offers a localized interface and a choice of light or dark appearance across the desktop and browser companion. Changes are relative to the v0.3.0 source milestone.

## Added

- English, Russian, French, German, Spanish and Italian interfaces for Focus, Websites, schedules, browser connection, settings and update controls. Choose a language by its native name in Settings → Appearance; the interface changes without restarting.
- A simple saved Light / Dark selector in the same settings section. Both palettes cover backgrounds, text, inputs, selections and status colors.
- The browser popup, connection page and block page follow the saved desktop language and theme after the next sync. Browser extension-manager names and descriptions have six native locale bundles.

## Improved

- Existing settings migrate to English and Light while retaining website lists, schedules, focus sessions, privacy behavior and update preferences.
- Personal website-list names, domains, file paths, IANA timezones and technical diagnostic details are preserved. Confirmation phrases remain `TAKE A BREAK` and `END FOCUS` in all languages.
- Catalog checks cover complete translations, parameter placeholders, generated browser files and embedded companion locale assets. Regression checks cover saved appearance, legacy settings, unsaved website edits and browser-policy propagation.

## Upgrade and downloads

Windows x64: `WebsiteBlocker-v0.4.0.exe`, optional `WebsiteBlocker-Companion-v0.4.0.zip`, and `checksums.txt`. Keep the new executable in a permanent folder. Prepare the browser connection again, then reload the managed companion in Chrome/Edge to pick up the new translations and appearance. Existing settings remain outside the executable directory.

The companion still requires the browser's explicit Developer mode / Load unpacked step; it is not distributed through browser stores. Builds are unsigned. Ordinary OS input, perceived smoothness, native picker interaction, mixed-DPI behavior and real update installation require separate manual verification. See [verification notes](VERIFICATION.md) for checks actually performed.
