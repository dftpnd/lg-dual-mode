# lg-dual-mode

Switch the **Dual-Mode** setting of an LG UltraGear 45GX950A monitor from Linux.

The 45GX950A can change the physical active area of its panel: either the full
5120×2160 @165 Hz ultrawide surface (104×44 cm), or a centred 16:9 37″ rectangle
of 3840×2160 @165 Hz (78×44 cm). Out of the box this is only reachable through
the on-screen display or through LG Switch, LG's own utility, which exists for
Windows and macOS but not for Linux.

This is a small Rust program that sends the same command the LG Switch app sends,
speaking DDC/CI over the i2c lines of the video cable.

```
lg-dual-mode status     print the current mode
lg-dual-mode wide       full width, 5120x2160@165
lg-dual-mode 16:9       16:9 37", 3840x2160@165
lg-dual-mode toggle     switch to the other one
```

## The protocol

The command is a DDC/CI *set VCP feature* request. There is no dependency on
`ddcutil`; the i2c transaction is implemented directly.

| | |
|---|---|
| VCP opcode | `0xB1` (`eMCCS_OPCODE_MONITOR_DUAL_MODE`) |
| Value | `(group << 8) \| index` |
| Group | `0x22` — Dual-Mode off, `0x21` — Dual-Mode on |

Indices for group `0x22` (Dual-Mode off): `0` full width, `1` 16:9 37″,
`2` 16:9 24.5″, `3` 16:9, `4` 24.5″.

Indices for group `0x21` (Dual-Mode on, high refresh rate): `0` full width,
`1` 21:9 39″, `2` 21:9 34″, `3` 16:9 37″, `4` 16:9 27″, `5` 16:9 24″.

So switching to 16:9 37″ means writing `0x2201` to VCP feature `0xB1`, and
returning to full width means writing `0x2200`. With `ddcutil` installed the
same thing can be done by hand:

```sh
ddcutil setvcp B1 x2201 --permit-unknown-feature --noverify
```

Two things make this feature hard to find. It is **write-only**, so diffing every
readable VCP value before and after a mode change shows no difference at all. And
it is **not advertised** in the monitor's capabilities string, which is why
`ddcutil` refuses it without `--permit-unknown-feature`.

The current mode is detected from the EDID product code — `40605` is full width,
`40616` is 16:9 37″ — because the monitor presents itself as a different device in
each mode. The i2c bus is located by walking `/dev/i2c-*` and matching the EDID
against the connected DRM connector, so changing video output or port does not
break anything.

## Where the opcodes came from

The LG Switch installer (InstallShield) was unpacked with
`wine LGSwitch_7.60.exe /extract_all:<dir>` followed by `unshield` — note that
`unshield` cannot read the installer directly, it needs the `data1.hdr` that
`/extract_all` writes out. The application turned out to be .NET, so
`ModeSwitchModule.dll` and `SwitchAppModel.dll` were decompiled with `ilspycmd`.
The opcodes are plain constants in the `MonitorOpcode.T_E_OPCODE` enum.

The USB HID device `043e:9a8a` ("LG Monitor Controls", which appears when the
monitor's USB-C upstream port is connected) is a red herring: it is used only by
the firmware update components, not by mode switching. It sends no unsolicited
input reports and stalls `GET_FEATURE` requests.

## Install

```sh
cargo build --release
sudo install -m 755 target/release/lg-dual-mode /usr/local/bin/
sudo install -m 644 udev/60-lg-dual-mode-i2c.rules /etc/udev/rules.d/
sudo usermod -aG i2c "$USER"
```

Group membership only takes effect on the next login. `status` works without it,
since it reads the EDID from sysfs rather than the i2c bus.

## Caveats

- Switching replaces the whole EDID, so the monitor reappears as a different
  device and the compositor re-applies its display configuration.
- Over **HDMI** this broke the link on the nvidia 595.84 driver
  (`nvidia-modeset: WARNING: GPU:0: HDMI FRL link training failed`) and the
  signal never came back. Over **DisplayPort** there is no such problem, because
  FRL is not used there. Use DisplayPort.
- The monitor's own Dual-Mode hardware button cannot produce the
  "full width ↔ 16:9 37″" pair: it only remembers modes from the "on" group.
- Only tested on one monitor, an LG UltraGear 45GX950A (internal model `WK95U`).
  Other LG models with Dual-Mode may well use the same opcode, but the EDID
  product codes used for detection are specific to this one.

## Licence

MIT. Not affiliated with or endorsed by LG Electronics.
