# Random Wheel

A wheel of fortune for Windows: type the participants in the panel on the right, press "Spin the wheel", and the app picks a winner.
Written in Rust (eframe/egui): a single exe with no dependencies that works offline.

The look follows Smart Switcher: the same light and dark palettes, the brand gradient `#4f6bff → #8a5bff`,
Segoe UI, cards, switches and capitalised section headings.

## Languages

The interface is available in English (default), Russian and Ukrainian. Pick the language in Settings → Interface; it applies immediately.
The starting language is taken from the app config, then from the `.nimvoro.json` marker next to the exe (written by the NIMVORO launcher), and falls back to English.

## Theme

Settings → Interface → Theme: System (default, follows Windows live), Light or Dark.

## Run

```bash
cargo run --release
```

The finished exe is `target/release/RandomWheel.exe`; you can copy it anywhere.

## Features

- **Wheel and list.** A List / Wheel switch above the wheel. In list mode the pick is shown as a running highlight.
- **Participants** are typed in the field on the right, one per line. Empty lines are skipped. Text pasted from other apps works.
- **Browse…** loads a list from `.txt` (one participant per line) or `.csv` (first column; separators `,` `;` or tab).
  Encodings: UTF-8, UTF-16 and Windows-1251, so a CSV from Excel on a Russian system works too.
- **Shuffle** and **A→Z** change the order of the segments.
- **Winner message** is your own phrase in the result dialog.
- **Spin duration** from 2 to 12 seconds.
- **Remove after picking** removes the winner from the list when you close the result dialog.
  Without it, the dialog offers a "Remove from list" button.
- **Confetti** when the winner is announced.
- **History** of the last 15 winners.
- **Full screen** with the button next to the switch or F11; Esc exits.

Shortcuts: Space or Enter spins (when the cursor is not in the input field); in the result dialog Enter, Space or Esc closes it.
You can also just click the wheel.

## Fair pick

The winner is chosen before the animation starts, with equal probability among all participants
(SplitMix64 seeded from the time and the system `RandomState`, no modulo bias).
The animation only brings the wheel to the chosen segment; the stopping point inside the segment is random too.

## Where settings are stored

`%APPDATA%\Random Wheel\config.json`: participants, settings, language, theme and history. Saved automatically.

## Layout

```
src/main.rs     window, icon (loaded from assets/icon.png)
src/app.rs      state, draw, main area, settings panel, winner dialog, confetti
src/strings.rs  interface strings (English, Russian, Ukrainian)
src/wheel.rs    random number generator, rotation maths, wheel painting
src/theme.rs    shared: light/dark palette, theme mode, widgets, icons
src/i18n.rs     shared: language, string tables, plurals, language/theme rows
src/nimvoro.rs  shared: update check and hand-off to the launcher
src/import.rs   reading .txt/.csv and detecting the encoding
src/config.rs   config.json
```

`theme.rs`, `i18n.rs` and `nimvoro.rs` are shared modules, copied byte for byte into the other Rust tools of the suite. Their API is described in the file headers.

## Tests

```bash
cargo test
```

For manual animation checks the debug build has the `RANDOM_WHEEL_AUTOSPIN=1` variable:
the wheel spins by itself one second after launch.

## Updates and NIMVORO

The tool is part of the [NIMVORO](https://github.com/GrigoryyKach/nimvoro) suite: the launcher installs it in one click and keeps it updated.
On startup the app checks the latest [release](https://github.com/GrigoryyKach/random-wheel/releases/latest) (the "Check on startup" switch under Updates).
If a new version exists, the "Update" button hands the update to the launcher, or opens the release page when there is no launcher.

A release is built by GitHub Actions when a tag is pushed: bump the version in `Cargo.toml`, then `git tag v{version} && git push --tags`.
