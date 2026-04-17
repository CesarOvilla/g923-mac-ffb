# Credits

## Author

**Cesar Ovilla** — reverse engineering, architecture, testing against real hardware (Mac mini M4 + Logitech G923 Xbox variant).

## Pair programming

Built in pair-programming with [Claude](https://claude.com/claude-code) (Anthropic). The assistant helped with protocol decoding, Rust scaffolding, and iterating on the FFB tuning. Every quirk listed in the README was validated on real hardware — not assumed.

## Protocol references

- [hid-logitech-hidpp.c](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c) — Linux kernel HID++ driver
- [new-lg4ff](https://github.com/berarma/new-lg4ff) — classic FFB driver for Linux
- [fffb](https://github.com/eddieavd/fffb) — FFB for G29/G923 PS on macOS (architectural inspiration)
- [SDL3 PR #11598](https://github.com/libsdl-org/SDL/pull/11598) — classic FFB in SDL3
