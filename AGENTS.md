# catlock — AI Agent Instructions

## Read this first

- This is a standalone Rust workspace, not part of zirofit-mono.
- `.ai/` conventions from zirofit-mono do not apply here, but the following
  carry over and are non-negotiable:
  - **No stubs in shipped code.** A feature ships only when wired end to end
    (daemon + IPC + GUI). Stubs are for tests only. The `TODO(Mn)` markers in
    the code are scaffolding, not shipped features.
  - **250-line file cap.** Split before you hit it.
  - **Conventional commits**: `type(scope): description`, explicit paths only.
  - **Run only the tests you touched.** Never `cargo test --workspace` as a
    gate; run `cargo test -p <crate>` for the crate you changed.
  - **SOLID / DRY / KISS / SOC.** Core crate stays pure (no I/O).

## Layout

- `crates/catlock-core` — pure logic: chord, config, state machine.
- `crates/catlock-input` — evdev: discovery, grab, hotplug, logind.
- `crates/catlockd` — system daemon + D-Bus service.
- `crates/catlock` — user binary: agent, settings (Slint), CLI.
- `spike/` — M0 throwaway. Deleted before v0.1.

## Dev loop

```sh
make build        # cargo build --workspace
make test         # cargo test -p catlock-core
make install      # sudo: install unit + binary (dev only)
make logs         # journalctl -fu catlockd
```

The daemon runs as a system service; restarting it releases and re-takes the
grab. The spike auto-ungrabs after 10 s so it can never brick a session.