// Shared library for the G923 userspace HID++ driver.
//
// Binaries that only need standalone "hello world" flows (like
// `g923-ping`, `g923-enumerate`) keep their own inline code. Everything
// from Fase 2b onward — feature discovery, FFB effects, input parsing —
// builds on top of the helpers in `hidpp`.

pub mod ffb;
pub mod hidpp;
pub mod telemetry;
