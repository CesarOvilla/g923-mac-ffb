// Client for HID++ 2.0 feature 0x8123 — ForceFeedback.
//
// Function indices and effect type IDs are taken verbatim from
// `hid-logitech-hidpp.c` in the Linux kernel (HIDPP_FF_* constants).
// Cross-checked against feature_id 0x8123 confirmed present at
// feature_idx=11 on the G923 Xbox via `g923-discover`.
//
// Slot allocation model: the host does NOT pick slots. DownloadEffect
// returns the device-assigned slot in response[0], and the host uses
// that slot id for SetEffectState / DestroyEffect.

use crate::hidpp::{Error, HidppDevice, FEATURE_ID_FORCE_FEEDBACK};

// Function indices within feature 0x8123.
pub const FN_GET_INFO: u8 = 0;
pub const FN_RESET_ALL: u8 = 1;
pub const FN_DOWNLOAD_EFFECT: u8 = 2;
pub const FN_SET_EFFECT_STATE: u8 = 3;
pub const FN_DESTROY_EFFECT: u8 = 4;
pub const FN_GET_APERTURE: u8 = 5;
pub const FN_SET_APERTURE: u8 = 6;
pub const FN_GET_GLOBAL_GAINS: u8 = 7;
pub const FN_SET_GLOBAL_GAINS: u8 = 8;

// Effect type IDs.
pub const EFFECT_CONSTANT: u8 = 0x00;
pub const EFFECT_PERIODIC_SINE: u8 = 0x01;
pub const EFFECT_PERIODIC_SQUARE: u8 = 0x02;
pub const EFFECT_PERIODIC_TRIANGLE: u8 = 0x03;
pub const EFFECT_PERIODIC_SAWTOOTH_UP: u8 = 0x04;
pub const EFFECT_PERIODIC_SAWTOOTH_DOWN: u8 = 0x05;
pub const EFFECT_SPRING: u8 = 0x06;
pub const EFFECT_DAMPER: u8 = 0x07;
pub const EFFECT_FRICTION: u8 = 0x08;
pub const EFFECT_INERTIA: u8 = 0x09;
pub const EFFECT_RAMP: u8 = 0x0A;

// Playback states for SetEffectState.
pub const STATE_STOP: u8 = 0;
pub const STATE_PLAY: u8 = 1;

/// OR-able into the effect type byte. When set, the effect starts
/// playing immediately on DownloadEffect — no separate SetEffectState
/// call is needed. The kernel uses this for the spring autocenter.
pub const EFFECT_AUTOSTART: u8 = 0x80;

#[derive(Debug, Clone, Copy)]
pub struct FfbInfo {
    /// Slot count as reported by the device. The kernel subtracts one
    /// "reserved" slot before exposing it as `num_effects`; we keep the
    /// raw value here and let the device pick slots for us.
    pub raw_slot_count: u8,
    pub raw_response: [u8; 16],
}

pub struct ForceFeedback<'a> {
    dev: &'a HidppDevice,
    index: u8,
}

impl<'a> ForceFeedback<'a> {
    /// Resolve the ForceFeedback feature index dynamically via IRoot.
    pub fn new(dev: &'a HidppDevice) -> Result<Self, Error> {
        let info = dev.get_feature(FEATURE_ID_FORCE_FEEDBACK)?;
        Ok(Self {
            dev,
            index: info.index,
        })
    }

    /// Skip discovery and use a known feature index. Useful when the
    /// caller already has the feature table cached.
    pub fn with_index(dev: &'a HidppDevice, index: u8) -> Self {
        Self { dev, index }
    }

    pub fn feature_index(&self) -> u8 {
        self.index
    }

    pub fn get_info(&self) -> Result<FfbInfo, Error> {
        let res = self.dev.send_sync(self.index, FN_GET_INFO, &[])?;
        Ok(FfbInfo {
            raw_slot_count: res[0],
            raw_response: res,
        })
    }

    /// Destroy every effect currently programmed on the wheel.
    pub fn reset_all(&self) -> Result<(), Error> {
        self.dev.send_sync(self.index, FN_RESET_ALL, &[])?;
        Ok(())
    }

    /// Upload a constant-force effect that starts playing immediately.
    /// Returns the device-assigned slot id.
    ///
    /// - `force`: signed 16-bit. Use the natural Linux ff_effect convention:
    ///   **positive = right turn, negative = left turn** from the driver's
    ///   perspective. Magnitude up to ~32767.
    /// - `duration_ms`: how long the effect runs. 0 = infinite. The slot
    ///   should still be released with `destroy()` when the host is done.
    ///
    /// G923 Xbox quirks vs the kernel hid-logitech-hidpp.c (G920) reference:
    ///
    /// 1. `SetEffectState(PLAY)` (function 3) is silently ignored by this
    ///    firmware — the device ACKs the request but never starts the
    ///    effect. The only working playback path is `EFFECT_AUTOSTART` on
    ///    DownloadEffect, which we set unconditionally here.
    ///
    /// 2. The wire convention for the force sign is **inverted** compared
    ///    to the kernel: on the G923 Xbox firmware, a positive wire force
    ///    rotates the wheel to the driver's LEFT. We negate internally so
    ///    callers see the standard convention.
    pub fn upload_constant(&self, force: i16, duration_ms: u16) -> Result<u8, Error> {
        self.upload_constant_envelope(force, duration_ms, 0, 0, 0, 0)
    }

    /// Same as `upload_constant` but with envelope shaping.
    ///
    /// The envelope ramps the force in and out:
    ///
    /// ```text
    ///   attack_level ─┐       ┌─ fade_level
    ///                  \     /
    ///         force ──  ─────  ── (sustain at `force` between attack and fade)
    ///                  |     |
    ///              attack  fade
    ///              length  length
    ///                ↕      ↕
    /// ```
    ///
    /// - `attack_level`: force at the very start (0 = start from zero,
    ///   255 = start at full). Ramps linearly to `force` over `attack_ms`.
    /// - `attack_ms`: duration of the ramp-up. 0 = instant.
    /// - `fade_level`: force at the very end. Fades from `force` to this
    ///   over `fade_ms` before the effect ends.
    /// - `fade_ms`: duration of the fade. 0 = instant stop.
    pub fn upload_constant_envelope(
        &self,
        force: i16,
        duration_ms: u16,
        attack_level: u8,
        attack_ms: u16,
        fade_level: u8,
        fade_ms: u16,
    ) -> Result<u8, Error> {
        let wire_force = force.saturating_neg();

        let mut p = [0u8; 14];
        p[1] = EFFECT_CONSTANT | EFFECT_AUTOSTART;
        p[2] = (duration_ms >> 8) as u8;
        p[3] = (duration_ms & 0xFF) as u8;
        let force_bits = wire_force as u16;
        p[6] = (force_bits >> 8) as u8;
        p[7] = (force_bits & 0xFF) as u8;
        p[8] = attack_level;
        p[9] = (attack_ms >> 8) as u8;
        p[10] = (attack_ms & 0xFF) as u8;
        p[11] = fade_level;
        p[12] = (fade_ms >> 8) as u8;
        p[13] = (fade_ms & 0xFF) as u8;
        let res = self.dev.send_sync(self.index, FN_DOWNLOAD_EFFECT, &p)?;
        Ok(res[0])
    }

    /// Upload a periodic sine effect for engine vibration.
    ///
    /// - `magnitude`: vibration amplitude (0 = no vibration, 32767 = max)
    /// - `period_ms`: period of one cycle in ms (e.g. 40ms = 25 Hz)
    /// - `offset`: DC offset (usually 0)
    ///
    /// Layout matches `hidpp_ff_upload_effect()` for `FF_PERIODIC` in the
    /// kernel: 14 bytes, same size as constant force.
    pub fn upload_periodic_sine(
        &self,
        magnitude: i16,
        period_ms: u16,
        offset: i16,
    ) -> Result<u8, Error> {
        let mut p = [0u8; 14];
        p[1] = EFFECT_PERIODIC_SINE | EFFECT_AUTOSTART;
        // p[2..3] = duration = 0 (infinite)
        // p[4..5] = delay = 0
        let mag = magnitude as u16;
        p[6] = (mag >> 8) as u8;
        p[7] = (mag & 0xFF) as u8;
        let off = offset as u16;
        p[8] = (off >> 8) as u8;
        p[9] = (off & 0xFF) as u8;
        p[10] = (period_ms >> 8) as u8;
        p[11] = (period_ms & 0xFF) as u8;
        // p[12..13] = phase = 0
        let res = self.dev.send_sync(self.index, FN_DOWNLOAD_EFFECT, &p)?;
        Ok(res[0])
    }

    /// GetGlobalGains → (gain, boost). Both u16 BE.
    /// Per kernel default-fallback, max gain is 0xFFFF.
    pub fn get_global_gains(&self) -> Result<(u16, u16), Error> {
        let res = self.dev.send_sync(self.index, FN_GET_GLOBAL_GAINS, &[])?;
        let gain = u16::from_be_bytes([res[0], res[1]]);
        let boost = u16::from_be_bytes([res[2], res[3]]);
        Ok((gain, boost))
    }

    pub fn set_global_gains(&self, gain: u16, boost: u16) -> Result<(), Error> {
        let p = [
            (gain >> 8) as u8,
            (gain & 0xFF) as u8,
            (boost >> 8) as u8,
            (boost & 0xFF) as u8,
        ];
        self.dev.send_sync(self.index, FN_SET_GLOBAL_GAINS, &p)?;
        Ok(())
    }

    /// GetAperture → rotation range in degrees (u16 BE).
    /// Kernel default fallback: 900.
    pub fn get_aperture(&self) -> Result<u16, Error> {
        let res = self.dev.send_sync(self.index, FN_GET_APERTURE, &[])?;
        Ok(u16::from_be_bytes([res[0], res[1]]))
    }

    pub fn set_aperture(&self, range_deg: u16) -> Result<(), Error> {
        let p = [(range_deg >> 8) as u8, (range_deg & 0xFF) as u8];
        self.dev.send_sync(self.index, FN_SET_APERTURE, &p)?;
        Ok(())
    }

    /// **Note**: silently ignored on G923 Xbox firmware. Kept for parity
    /// with the HID++ spec and possible future use on other variants.
    /// On the G923 Xbox, force playback is started via the `EFFECT_AUTOSTART`
    /// bit on DownloadEffect (see `upload_constant`).
    pub fn play(&self, slot: u8) -> Result<(), Error> {
        self.dev
            .send_sync(self.index, FN_SET_EFFECT_STATE, &[slot, STATE_PLAY])?;
        Ok(())
    }

    /// **Note**: see `play()` — silently ignored on G923 Xbox.
    pub fn stop(&self, slot: u8) -> Result<(), Error> {
        self.dev
            .send_sync(self.index, FN_SET_EFFECT_STATE, &[slot, STATE_STOP])?;
        Ok(())
    }

    pub fn destroy(&self, slot: u8) -> Result<(), Error> {
        self.dev
            .send_sync(self.index, FN_DESTROY_EFFECT, &[slot])?;
        Ok(())
    }

    /// Upload a condition effect (SPRING / DAMPER / FRICTION / INERTIA).
    ///
    /// Byte layout is a verbatim port of `hidpp_ff_upload_effect()` for
    /// the `FF_SPRING`-family cases in hid-logitech-hidpp.c. Total 18
    /// bytes of params, sent over the HID++ very-long report.
    ///
    /// `AUTOSTART` is set unconditionally — see `upload_constant` for
    /// the G923 Xbox quirks.
    ///
    /// Sign handling: the `center` field is an absolute position, so it
    /// inherits the same sign flip as constant force (G923 Xbox wire
    /// convention is inverted vs kernel). Coefficients are **stiffness**,
    /// not direction, so they are NOT inverted — a positive coefficient
    /// always means "pull toward center" on both the kernel and the wire.
    /// Inverting coefficients turns a spring into a destabilizing negative
    /// spring that pushes the wheel to the end stops.
    pub fn upload_condition(
        &self,
        effect_type: u8,
        left_coeff: i16,
        right_coeff: i16,
        left_saturation: u16,
        right_saturation: u16,
        deadband: u16,
        center: i16,
    ) -> Result<u8, Error> {
        let left_coeff_wire = left_coeff as u16;
        let right_coeff_wire = right_coeff as u16;
        let center_wire = center.saturating_neg() as u16;

        let mut p = [0u8; 18];
        // p[0] = slot (0 = let device pick)
        p[1] = effect_type | EFFECT_AUTOSTART;
        // p[2..5] = duration + delay = 0 (infinite, no delay)

        // left_saturation: upper 15 bits stored, BE
        p[6] = (left_saturation >> 9) as u8;
        p[7] = ((left_saturation >> 1) & 0xFF) as u8;
        // left_coeff: i16 BE
        p[8] = (left_coeff_wire >> 8) as u8;
        p[9] = (left_coeff_wire & 0xFF) as u8;
        // deadband: upper 15 bits BE
        p[10] = (deadband >> 9) as u8;
        p[11] = ((deadband >> 1) & 0xFF) as u8;
        // center: i16 BE
        p[12] = (center_wire >> 8) as u8;
        p[13] = (center_wire & 0xFF) as u8;
        // right_coeff: i16 BE
        p[14] = (right_coeff_wire >> 8) as u8;
        p[15] = (right_coeff_wire & 0xFF) as u8;
        // right_saturation: upper 15 bits BE
        p[16] = (right_saturation >> 9) as u8;
        p[17] = ((right_saturation >> 1) & 0xFF) as u8;

        let res = self.dev.send_sync(self.index, FN_DOWNLOAD_EFFECT, &p)?;
        Ok(res[0])
    }

    /// Symmetric spring autocenter. Both sides use the same coefficient
    /// and saturation; center = 0; no deadband. `coefficient` controls
    /// stiffness — higher = harder to push off center and faster return.
    pub fn upload_spring(&self, coefficient: i16, saturation: u16) -> Result<u8, Error> {
        self.upload_condition(
            EFFECT_SPRING,
            coefficient,
            coefficient,
            saturation,
            saturation,
            0,
            0,
        )
    }

    /// Symmetric damper. Resists angular velocity — higher coefficient
    /// = wheel feels "heavier" when rotated quickly.
    pub fn upload_damper(&self, coefficient: i16, saturation: u16) -> Result<u8, Error> {
        self.upload_condition(
            EFFECT_DAMPER,
            coefficient,
            coefficient,
            saturation,
            saturation,
            0,
            0,
        )
    }

    pub fn upload_friction(&self, coefficient: i16, saturation: u16) -> Result<u8, Error> {
        self.upload_condition(
            EFFECT_FRICTION,
            coefficient,
            coefficient,
            saturation,
            saturation,
            0,
            0,
        )
    }

    pub fn upload_inertia(&self, coefficient: i16, saturation: u16) -> Result<u8, Error> {
        self.upload_condition(
            EFFECT_INERTIA,
            coefficient,
            coefficient,
            saturation,
            saturation,
            0,
            0,
        )
    }
}
