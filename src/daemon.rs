// Daemon FFB: telemetría ATS → fuerza en el G923.
//
// Lee configuración de g923.toml, telemetría de shared memory,
// y envía efectos FFB al volante via HID++ con hidapi shared-device.
// Hot-reload: detecta cambios en g923.toml cada 5 segundos.

use g923_mac_ffb::config::{self, ConfigLoader};
use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use g923_mac_ffb::telemetry::TelemetryReader;
use hidapi::HidApi;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Daemon FFB G923 v0.1");
    println!();

    // ── Config ───────────────────────────────────────────────────
    let config_path = std::env::args().nth(1);
    let mut loader = ConfigLoader::new(config_path.as_deref());
    println!("✓ Configuración: {}", loader.path_display());

    // Si no existe archivo, generar uno por defecto
    if loader.path_display() == "(defaults)" {
        let default_path = "g923.toml";
        if !std::path::Path::new(default_path).exists() {
            std::fs::write(default_path, config::generate_default_toml())?;
            println!("  → Generado {default_path} con valores por defecto.");
            println!("  → Edítalo para ajustar el FFB. Se recarga automáticamente.");
            // Re-cargar ahora que existe
            loader = ConfigLoader::new(Some(default_path));
        }
    }

    let cfg = &loader.config.ffb;
    println!(
        "  spring: base={} per_kmh={} max={}",
        cfg.spring.base, cfg.spring.per_kmh, cfg.spring.max
    );
    println!(
        "  lateral: gain={} max={} smoothing={}",
        cfg.lateral.gain, cfg.lateral.max, cfg.lateral.smoothing
    );
    println!(
        "  damper: base={} per_kmh={} max={}",
        cfg.damper.base, cfg.damper.per_kmh, cfg.damper.max
    );
    println!();

    // ── Telemetría ───────────────────────────────────────────────
    println!("   Esperando telemetría de ATS...");
    let mut telem = loop {
        match TelemetryReader::open() {
            Ok(r) => break r,
            Err(_) => sleep(Duration::from_secs(2)),
        }
    };
    println!("✓ Telemetría conectada.");

    // ── Wheel ────────────────────────────────────────────────────
    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;
    println!("✓ G923 FFB conectado (feature_idx={}).", ffb.feature_index());
    println!();
    println!("   Maneja en ATS. Ctrl+C para salir.");
    println!("   Edita g923.toml para ajustar — recarga automática cada 5s.");
    println!();

    // ── Estado ───────────────────────────────────────────────────
    let mut spring_slot: Option<u8> = None;
    let mut damper_slot: Option<u8> = None;
    let mut lateral_slot: Option<u8> = None;
    let mut vibration_slot: Option<u8> = None;
    let mut last_spring: f32 = 0.0;
    let mut last_damper: f32 = 0.0;
    let mut last_lateral: f32 = 0.0;
    let mut last_vib_mag: f32 = 0.0;
    let mut smoothed_lateral: f32 = 0.0;
    let mut last_status = Instant::now();
    let mut last_config_check = Instant::now();
    let mut was_paused = true;

    loop {
        let loop_start = Instant::now();

        // ── Hot-reload config cada 5 segundos ────────────────────
        if last_config_check.elapsed() > Duration::from_secs(5) {
            if loader.check_reload() {
                println!("  ↻ Configuración recargada desde {}", loader.path_display());
            }
            last_config_check = Instant::now();
        }

        let cfg = &loader.config.ffb;
        let loop_period = Duration::from_millis(1000 / cfg.update_hz.max(1) as u64);

        if !telem.has_new_frame() {
            sleep(Duration::from_millis(2));
            continue;
        }

        let t = telem.read();
        let gain = cfg.global_gain;

        // ── Pausa ────────────────────────────────────────────────
        if t.paused != 0 {
            if !was_paused {
                if let Some(s) = spring_slot.take() { let _ = ffb.destroy(s); }
                if let Some(s) = damper_slot.take() { let _ = ffb.destroy(s); }
                if let Some(s) = lateral_slot.take() { let _ = ffb.destroy(s); }
                if let Some(s) = vibration_slot.take() { let _ = ffb.destroy(s); }
                last_spring = 0.0;
                last_damper = 0.0;
                last_lateral = 0.0;
                last_vib_mag = 0.0;
                smoothed_lateral = 0.0;
                was_paused = true;
                println!("  ⏸ pausa");
            }
            sleep(Duration::from_millis(200));
            continue;
        }
        if was_paused {
            println!("  ▶ activo");
            was_paused = false;
        }

        let speed_kmh = t.speed * 3.6;

        // ── Spring ───────────────────────────────────────────────
        let coeff = ((cfg.spring.base + speed_kmh * cfg.spring.per_kmh).min(cfg.spring.max) * gain) as f32;
        if (coeff - last_spring).abs() > cfg.spring.threshold || spring_slot.is_none() {
            if let Ok(s) = ffb.upload_spring(coeff as i16, 0xFFFF) {
                if let Some(old) = spring_slot { let _ = ffb.destroy(old); }
                spring_slot = Some(s);
                last_spring = coeff;
            }
        }

        // ── Damper ───────────────────────────────────────────────
        let damp = ((cfg.damper.base + speed_kmh * cfg.damper.per_kmh).min(cfg.damper.max) * gain) as f32;
        if (damp - last_damper).abs() > cfg.damper.threshold || damper_slot.is_none() {
            if let Ok(s) = ffb.upload_damper(damp as i16, 0xFFFF) {
                if let Some(old) = damper_slot { let _ = ffb.destroy(old); }
                damper_slot = Some(s);
                last_damper = damp;
            }
        }

        // ── Lateral ──────────────────────────────────────────────
        let raw = (t.accel_x * cfg.lateral.gain * gain).clamp(-cfg.lateral.max, cfg.lateral.max);
        smoothed_lateral = smoothed_lateral * cfg.lateral.smoothing + raw * (1.0 - cfg.lateral.smoothing);
        let lat = smoothed_lateral;

        if (lat - last_lateral).abs() > cfg.lateral.threshold {
            if lat.abs() > cfg.lateral.deadzone {
                if let Ok(s) = ffb.upload_constant(lat as i16, 200) {
                    if let Some(old) = lateral_slot { let _ = ffb.destroy(old); }
                    lateral_slot = Some(s);
                }
            } else if let Some(old) = lateral_slot.take() {
                let _ = ffb.destroy(old);
            }
            last_lateral = lat;
        }

        // ── Vibración del motor ────────────────────────────────────
        let mut vib_mag: f32 = 0.0;
        let mut vib_period: u16 = 30;
        if cfg.vibration.enabled && t.rpm > 100.0 {
            // Amplitud: idle_amplitude en idle, sube con RPM hasta max_amplitude
            let rpm_ratio = ((t.rpm - 400.0) / 2000.0).clamp(0.0, 1.0);
            vib_mag = (cfg.vibration.idle_amplitude
                + rpm_ratio * (cfg.vibration.max_amplitude - cfg.vibration.idle_amplitude))
                * cfg.vibration.rpm_gain * gain;
            vib_mag = vib_mag.min(cfg.vibration.max_amplitude);

            // Período: simula frecuencia de encendido de 6 cilindros
            // firing_freq = RPM / 40 → period = 40000 / RPM
            vib_period = (40000.0 / t.rpm).clamp(10.0, 100.0) as u16;

            if (vib_mag - last_vib_mag).abs() > 200.0 || vibration_slot.is_none() {
                if let Ok(s) = ffb.upload_periodic_sine(vib_mag as i16, vib_period, 0) {
                    if let Some(old) = vibration_slot { let _ = ffb.destroy(old); }
                    vibration_slot = Some(s);
                    last_vib_mag = vib_mag;
                }
            }
        } else if vibration_slot.is_some() {
            if let Some(old) = vibration_slot.take() { let _ = ffb.destroy(old); }
            last_vib_mag = 0.0;
        }

        // ── Status ───────────────────────────────────────────────
        if last_status.elapsed() > Duration::from_secs(3) {
            println!(
                "  {:>6.1} km/h | {:>5.0} rpm | spr {:>5} | dmp {:>5} | lat {:>+6} | vib {:>4} {:>2}ms",
                speed_kmh, t.rpm, coeff as i16, damp as i16, lat as i16,
                vib_mag as i16, vib_period,
            );
            last_status = Instant::now();
        }

        // ── Rate limit ───────────────────────────────────────────
        let elapsed = loop_start.elapsed();
        if elapsed < loop_period {
            sleep(loop_period - elapsed);
        }
    }
}
