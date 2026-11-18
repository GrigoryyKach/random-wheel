# Random Wheel

A wheel of fortune for Windows and Linux: type the participants in the panel on the right, press "Spin the wheel", and the app picks a winner.
Written in Rust (eframe/egui): a single executable with no dependencies that works offline.

The look follows Smart Switcher: the same light and dark palettes, the brand gradient `#4f6bff → #8a5bff`,
Segoe UI on Windows (the bundled font on Linux), cards, switches and capitalised section headings.

## Languages

The interface is available in English (default), Russian and Ukrainian. Pick the language in Settings → Interface; it applies immediately.
The starting language is taken from the app config, then from the `.nimvoro.json` marker next to the exe (written by the NIMVORO launcher), and falls back to English.

## Theme

Settings → Interface → Theme: System (default, follows the Windows setting live; on Linux it follows the Wayland portal or GNOME `color-scheme`, falling back to dark), Light or Dark.

## Platforms

- **Windows** 10/11 x64: `RandomWheel-win-x64.zip` from the release, or via the NIMVORO launcher.
- **Linux** x86_64, X11 and Wayland (built on Ubuntu 22.04, so glibc 2.35 or newer): `RandomWheel-linux-x64.tar.gz` from the release, or via the NIMVORO launcher.

## Run

```bash
cargo run --release
```

The finished exe is `target/release/RandomWheel.exe` (`target/release/RandomWheel` on Linux); you can copy it anywhere.

### Linux

Unpack the archive and run `./random-wheel`. The file dialog goes through `xdg-desktop-portal` (present on all mainstream desktops; install `xdg-desktop-portal-gtk` or `-kde` if the Browse button does nothing). To build from source:

```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev
cargo build --release
```

Known limitations: on Wayland the window is positioned by the compositor (the "centered" hint is ignored); some systems lack an emoji font, so the emoji in the winner message may show as a box.

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
- **Dead by Daylight tab** with house rules; see below.

Shortcuts: Space or Enter spins (when the cursor is not in the input field); in the result dialog Enter, Space or Esc closes it.
You can also just click the wheel.

## Dead by Daylight mode

The second tab, "Dead by Daylight", picks which killer you play, with a set of house rules built in. The classic wheel is unchanged. The last open tab is remembered.
There is one state per PC: your pool, ban counters, immunity, the last winner and the history.

1. **Pool.** Check the killers who take part. The picker has search and All, None and Invert buttons.
2. **Bans.** Friends ban killers from the pool for this round. A ban can be undone until you spin.
3. **Spin.** The wheel spins over *pool minus banned minus the last winner*. The result ends the round and commits its effects.

Rules:

- **Last winner sits out.** The killer who won the previous round is locked for exactly one round: it is greyed out, cannot be added to the pool and cannot be banned. Then the lock moves to the new winner.
- **Two bans earn immunity.** Every committed round adds 1 to the ban counter of each killer banned in it (shown as two pips). When a counter reaches 2, the killer becomes immune for the next round (its ban button is disabled) and the counter resets to 0. Immunity ends after that round.
- **Guard.** With no killer left on the wheel, Spin is disabled and a hint says why. A single eligible killer can still be spun.
- **Undo last round** restores the state from before the last spin (one step). **Reset counters** clears bans, immunity and the lock after a confirmation.
- **History** keeps the last 50 rounds: date and time, pool size, banned killers and the winner.

The killer list is `data/dbd_killers.json`: every killer released so far, in release order, with the English name and the Russian name from the game's Russian localization. Killers that are no longer sold (for example The Shape) are marked and still selectable. Russian and Ukrainian interfaces both show the Russian names.
On start the app fetches the list from this repository in the background; if its `version` is newer it is cached in the settings folder and used, otherwise the cache or the built-in list is used. Until the list is updated you can add custom killers (one name for all languages) and remove them again.
The state is stored in `dbd.json` in the settings folder (see below), written atomically. Space spins when nothing is being edited.

This is an unofficial fan tool, not affiliated with Behaviour Interactive. It uses no game art or logos; "Dead by Daylight" appears only as a plain-text name.

## Fair pick

The winner is chosen before the animation starts, with equal probability among all participants
(SplitMix64 seeded from the time and the system `RandomState`, no modulo bias).
The animation only brings the wheel to the chosen segment; the stopping point inside the segment is random too.

## Where settings are stored

`%APPDATA%\Random Wheel\config.json` on Windows, `~/.config/Random Wheel/config.json` on Linux: participants, settings, language, theme and history. Saved automatically.

## Layout

```
src/main.rs     window, icon (loaded from assets/icon.png)
src/app.rs      state, draw, main area, settings panel, winner dialog, confetti
src/app/dbd_ui.rs  the Dead by Daylight tab: picker, round panel, history
src/dbd.rs      Dead by Daylight rules engine and killer list (pure logic, unit tests)
src/dbd_io.rs   dbd.json, killer list cache and background update
src/dbd_strings.rs  strings of the Dead by Daylight tab
data/dbd_killers.json  the killer list, also fetched from GitHub by running apps
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

A release is built by GitHub Actions when a tag is pushed (a Windows job and a Linux job, one release with both assets): bump the version in `Cargo.toml`, then `git tag v{version} && git push --tags`.

## Support

If you find this app useful, you can support its development: <https://donatello.to/MicsGame>
