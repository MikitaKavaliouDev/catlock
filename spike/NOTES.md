# M0 spike — experiment checklist

Run as root: `sudo ./target/debug/catlock-spike --seconds 10`
(or `sudo ./target/debug/catlock-spike --list` to inspect devices first).

The grab auto-releases after 10 s (or on kill -9). Type on the locked keyboard
while grabbed and observe whether anything reaches the desktop.

## Experiments

- [ ] E1: grab → type in Kate → release → type again, ×10 cycles. Does KWin
      recover input every time on Plasma 6 Wayland?
- [ ] E2: Meta, Ctrl+Alt+T, Ctrl+Alt+F2 while grabbed. Are they dead? Do they
      work again after release?
- [ ] E3: plug a USB keyboard while grabbed. Is the new device grabbed too?
      (Expected: no → confirms udev monitor is mandatory.)
- [ ] E4: `sudo kill -9 <pid>` while grabbed. Does the keyboard come back?
- [ ] E5: let the screen idle toward the locker while grabbed. Does the locker
      appear and block you? (Expected: yes → validates logind idle inhibitor.)
- [ ] E6: Slint window on Plasma 6 Wayland — SSD vs CSD, palette override
      coverage, kdeglobals font resolution. (Separate spike binary.)

## Results

(fill in after running)