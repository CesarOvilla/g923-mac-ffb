🇪🇸 [Leer en español](README.es.md)

# g923-mac-ffb

**Force Feedback for the Logitech G923 Racing Wheel (Xbox) on macOS Apple Silicon.**

100% userspace driver — no DriverKit, no kexts, no paid developer account. Works with American Truck Simulator and Euro Truck Simulator 2.

## The Problem

The Logitech G923 Xbox variant (PID `0xc26e`) has no official Force Feedback support on macOS. Logitech never updated their drivers for Apple Silicon, and macOS dropped kext support needed by the legacy `ForceFeedback.framework`. The wheel works as a joystick (axes, pedals, buttons), but the FFB motors are completely dead.

## The Solution

This project communicates directly with the G923 via **HID++ 4.2** (Logitech's proprietary protocol) from userspace, using `hidapi` over `IOHIDManager`. A telemetry plugin runs inside the game, publishes data to shared memory, and a separate daemon translates that telemetry into FFB commands sent to the wheel.

```
┌──────────────┐   telemetry    ┌─────────────────┐   HID++ 4.2   ┌───────┐
│ ATS / ETS2   │──────────────▶ │ g923-daemon     │──────────────▶ │ G923  │
│ (plugin .dylib) POSIX shm    │ (Rust, arm64)   │  USB reports   │ Xbox  │
└──────────────┘               └─────────────────┘               └───────┘
```

## Supported FFB Effects

| Effect | Description | Use |
|--------|-------------|-----|
| **Spring** | Speed-proportional centering | Faster = stiffer wheel |
| **Damper** | Angular velocity resistance | Anti-oscillation, smooths input |
| **Constant Force** | Lateral force in turns | Feel the G-forces in curves |
| **Periodic Sine** | Engine vibration by RPM | Feel the engine rumble |
| **Bumps** | Suspension deflection pulses | Road bumps and surface changes |
| **Weight** | Cargo mass multiplier | Loaded truck = heavier steering |
| Friction | Constant drag | Available in the library |
| Inertia | Virtual mass | Available in the library |

## Requirements

- **Apple Silicon Mac** (M1, M2, M3, M4)
- **macOS Sonoma 14+** (tested on macOS 26.4/Tahoe)
- **Logitech G923 Racing Wheel**, **Xbox/PC** variant (PID `0xc26e`)
- **Rust** toolchain (to build from source)
- **American Truck Simulator** or **Euro Truck Simulator 2** via Steam
- **clang** (included with Xcode Command Line Tools) for the C plugin

> **Note**: this project targets the **Xbox** variant of the G923 (`0xc26e`), not the PlayStation variant (`0xc266`). The PS variant uses a different protocol — use [fffb](https://github.com/eddieavd/fffb) for that one.

## Quick Install (from DMG)

Download the latest `.dmg` from [Releases](../../releases), open it, and double-click **"Instalar.command"**. It copies all binaries, sets up the config, and installs the auto-start service. Then just open ATS and drive.

## Build from Source

### 1. Compile

```bash
git clone https://github.com/your-user/g923-mac-ffb.git
cd g923-mac-ffb
cargo build --release
```

### 2. Install the telemetry plugin in ATS

The plugin is an x86_64 `.dylib` loaded by the game process:

```bash
# Build the plugin
bash plugin/build.sh

# Copy to ATS plugins directory
# If macOS blocks the copy, do it manually from Finder:
# Right-click ATS.app → Show Package Contents → Contents/MacOS → create "plugins" folder
cp plugin/g923_telemetry.dylib \
  ~/Library/Application\ Support/Steam/steamapps/common/\
  American\ Truck\ Simulator/American\ Truck\ Simulator.app/\
  Contents/MacOS/plugins/
```

> On first launch with the plugin, ATS shows an SDK warning dialog. This is normal — accept and continue.

### 3. Set up the daemon

```bash
# First run generates g923.toml with documented defaults
./target/release/g923-daemon
# (Ctrl+C after verifying the config file is created)

# Optionally move config to the standard location
mkdir -p ~/.config/g923
mv g923.toml ~/.config/g923/
```

### 4. Install as a service (auto-start)

```bash
./target/release/g923 install-service
./target/release/g923 start
./target/release/g923 status
```

## Usage

### With service installed (recommended)

1. Turn on your Mac (daemon starts automatically)
2. Open ATS/ETS2 from Steam
3. Drive — FFB activates automatically when telemetry is detected

### Manual

```bash
./target/release/g923-daemon
# Then open ATS in another window
```

### CLI

```bash
g923 start              # Start daemon in background
g923 stop               # Stop daemon
g923 status             # Show status
g923 log                # Tail the daemon log
g923 install-service    # Auto-start on login
g923 uninstall-service  # Remove auto-start
g923 uninstall          # Remove everything (binaries, config, service)
```

### Menu Bar App (optional)

A lightweight menu bar utility is included. It shows a green/red icon indicating daemon status, and lets you start/stop, open config, or view logs — all without opening Terminal.

```bash
~/.local/bin/G923FFB &
```

## Configuration

Edit `g923.toml` (in `~/.config/g923/` or the current directory). **The daemon hot-reloads changes every 5 seconds** — no restart needed.

```toml
[ffb]
global_gain = 1.0          # global multiplier (0.0–2.0)
update_hz = 15             # daemon update rate

[ffb.spring]
base = 2000                # centering force when stopped
per_kmh = 150              # increase per km/h
max = 18000                # maximum

[ffb.damper]
base = 1000                # base damping
per_kmh = 80               # increase per km/h
max = 10000                # maximum

[ffb.lateral]
gain = 2000                # lateral force intensity in curves
max = 10000                # maximum
smoothing = 0.3            # smoothing factor (0.0–0.9)

[ffb.vibration]
enabled = true             # engine vibration by RPM
rpm_gain = 0.5             # intensity
idle_amplitude = 500       # vibration at idle
max_amplitude = 3000       # vibration at high RPM

[ffb.surface]
enabled = true             # bumps from suspension deflection
bump_gain = 1.0            # intensity
bump_threshold = 0.015     # sensitivity (raise if false positives on smooth roads)

[ffb.weight]
enabled = true             # heavier cargo = heavier steering
reference_mass = 20000     # reference mass in kg
max_multiplier = 1.8       # maximum weight multiplier
```

## Diagnostic Tools

```bash
cargo run --bin g923-enumerate       # List G923 HID collections
cargo run --bin g923-ping            # HID++ protocol version check
cargo run --bin g923-discover        # Firmware feature table
cargo run --bin g923-constant-force  # Test constant force effect
cargo run --bin g923-spring          # Test spring effect
cargo run --bin g923-damper          # Test damper effect
cargo run --bin g923-input           # Real-time input viewer
cargo run --bin g923-telemetry-monitor  # Real-time ATS telemetry
```

## G923 Xbox Quirks on macOS

These behaviors are specific to the Xbox variant (`0xc26e`) and were discovered during development:

1. **`SetEffectState(PLAY)` is silently ignored** — the firmware ACKs but never activates. Effects must use the `EFFECT_AUTOSTART (0x80)` bit on `DownloadEffect`.

2. **Force sign is inverted** — the Linux kernel (G920) uses `+force = right`. The G923 Xbox uses `+force = left`. The library compensates internally.

3. **Replies on report ID `0x12`** — the G923 responds with very-long reports (64 bytes) even when the request was sent as long (20 bytes).

4. **`hidapi` requires `macos-shared-device`** — without this feature flag, `hidapi` opens in exclusive mode, disconnecting `GameController.framework` and killing game input. This was the most critical discovery of the project.

5. **Vendor collection `0xFF43` invisible under Rosetta** — x86_64 processes (like ATS) cannot see the HID++ collections. FFB must run from a native arm64 daemon, not from an in-process plugin.

6. **`SetAperture` (lock rotation range) is ignored** — the firmware accepts but doesn't change the physical range.

## Known Limitations

- Only works with the **Xbox** variant of the G923 (`0xc26e`)
- Only tested with **ATS** (ETS2 should work identically — same SDK)
- The telemetry plugin requires write access to the ATS `.app` bundle
- No classic lg4ff protocol support (G29/G920/G923 PS use a different protocol)
- Rotation range lock not available (use Logitech GHub instead)

## References

- [hid-logitech-hidpp.c](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c) — Linux kernel HID++ driver (primary protocol reference)
- [new-lg4ff](https://github.com/berarma/new-lg4ff) — Classic FFB driver for Linux (NOT compatible with G923 Xbox)
- [fffb](https://github.com/eddieavd/fffb) — FFB for G29/G923 PS on macOS (classic protocol, architectural inspiration)
- [SDL3 PR #11598](https://github.com/libsdl-org/SDL/pull/11598) — Classic FFB in SDL3

## License

MIT
