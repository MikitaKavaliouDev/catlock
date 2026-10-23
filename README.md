# catlock

Keyboard lock for Linux. One shortcut locks the whole keyboard; the same shortcut
unlocks it. Built for the "cleaning the keyboard / cat on the desk / toddler"
use case. Linux only — Windows and macOS already have equivalents.

## Escape hatches (read this first)

catlock grabs the keyboard at the kernel input layer. If something goes wrong you
are never permanently locked out:

1. **Unlock chord** (default `Ctrl+Alt+Shift+K`). Hold for 1.2 s to unlock.
2. **Deadman timer** — auto-unlocks after 2 minutes by default.
3. **Power key** — pressing the power button force-unlocks (consumed once).
4. **From another machine / TTY** — `systemctl stop catlockd`. The grab is tied
   to the daemon's open file descriptor, so the moment the process dies the
   kernel releases the keyboard. This works even over SSH.
5. **Hard reboot** always works; SysRq may work depending on your sysctl mask.

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

Early development. M0 (evdev grab/unlock spike) is the current milestone.
See `spike/` (deleted before v0.1).

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