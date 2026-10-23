# catlock

Keyboard lock for Linux. One shortcut locks the whole keyboard; the same shortcut
unlocks it. Built for the "cleaning the keyboard / cat on the desk / toddler"
use case. Linux only — Windows and macOS already have equivalents.

## Escape hatches (read this first)

catlock grabs the keyboard at the kernel input layer. If something goes wrong you
are never permanently locked out:

1. **Unlock chord** (default `Ctrl+Alt+Shift+K`). Unlocks instantly; set
   `hold_to_unlock_ms` if you want a hold-to-unlock delay.
2. **Hold `Esc` for 2 s** — built-in panic that works even if the configured
   chord is wrong or the config failed to parse.
3. **Deadman timer** — auto-unlocks after 2 minutes by default.
4. **Power key** — pressing the power button force-unlocks (consumed once).
5. **From another machine / TTY** — `systemctl stop catlockd`. The grab is tied
   to the daemon's open file descriptor, so the moment the process dies the
   kernel releases the keyboard. This works even over SSH.
6. **Hard reboot** always works.

> **Never press `Alt+SysRq+K` (SAK).** It is not a catlock escape hatch: it
> kills every session on the seat, and the display manager then respawns
> greeters on new VTs, which looks like a full-screen black console with a
> blinking cursor. It has already cost two hard shutdowns during testing.

## How it works

catlock reads `/dev/input/event*` and, while locked, takes an exclusive
`EVIOCGRAB` on every keyboard-like device. The compositor, X11/Wayland clients,
TTYs and screen lockers all stop receiving events, but catlock keeps reading
them, so it can still hear the unlock chord. This is the same mechanism
`keyd` / `evsieve` / `interception-tools` use, and it works identically on
Wayland and X11 because it sits below both.

## Privilege model

`catlockd` runs as a dedicated `catlock` system user in the `input` group via a
hardened systemd unit (`NoNewPrivileges`, `ProtectSystem=strict`, …). Your
account is never added to the `input` group, so ordinary apps cannot read your
keystrokes. The GUI and tray agent run in your session and talk to the daemon
over the system D-Bus (polkit-gated).

## Status

Early development. M1a: the daemon locks/unlocks, with deadman, Esc panic, VT
watchdog and re-lock cooldown. The systemd unit, D-Bus API and Slint settings
GUI are M1b+. See `spike/` (deleted before v0.1).

## Testing safely

Run the daemon as root in a real terminal. It mirrors its log to
`/var/log/catlockd.log` (persistent across reboots) and to stderr:

```sh
sudo ./target/debug/catlockd --dry-run --debug-keys --deadman 10
```

`--dry-run` reads and matches keys but never grabs — the keyboard stays live, so
you can verify the chord, the Esc panic and the deadman with zero risk. When that
looks right, drop `--dry-run` for a real grab test:

```sh
sudo ./target/debug/catlockd --deadman 10 --hold 0 --debug-keys
tail -f /var/log/catlockd.log   # second terminal
```

Startup logs a warning if the kernel SAK is enabled. Do not press
`Alt+SysRq+K`; hold `Esc` instead.

## Build

```sh
cargo build --workspace
cargo test -p catlock-core
```

## Security note

A daemon that reads every keystroke is a keylogger by capability. catlock never
logs key events (only the configured chord is matched and discarded), the code
is small and auditable, and there is no network access. See the threat model in
`docs/threat-model.md` (TODO).