// Fase 4 — daemon FFB: telemetría ATS → fuerza en el G923.
//
// Lee shared memory del plugin y traduce velocidad + fuerzas laterales
// a efectos FFB. Usa tasa baja (~15 Hz) y change detection para no
// saturar el firmware del G923 (USB Full Speed, procesador limitado).

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use g923_mac_ffb::telemetry::TelemetryReader;
use hidapi::HidApi;
use std::thread::sleep;
use std::time::{Duration, Instant};

// ── Parámetros de tuning ────────────────────────────────────────────

const LOOP_PERIOD: Duration = Duration::from_millis(66); // ~15 Hz

// Spring (autocentrado)
const SPRING_BASE: f32 = 2_000.0;
const SPRING_PER_KMH: f32 = 150.0;
const SPRING_MAX: f32 = 18_000.0;
const SPRING_SAT: u16 = 0xFFFF;
const SPRING_THRESHOLD: f32 = 1_500.0;

// Damper (anti-oscillation) — se activa a velocidad
const DAMPER_BASE: f32 = 1_000.0;
const DAMPER_PER_KMH: f32 = 80.0;
const DAMPER_MAX: f32 = 10_000.0;
const DAMPER_THRESHOLD: f32 = 1_000.0;

// Lateral force (curvas)
const LATERAL_GAIN: f32 = 2_000.0;
const LATERAL_MAX: f32 = 10_000.0;
const LATERAL_THRESHOLD: f32 = 500.0;
const LATERAL_DEADZONE: f32 = 300.0;
const LATERAL_SMOOTHING: f32 = 0.3; // 0.0 = sin filtro, 1.0 = sin cambio

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Daemon FFB G923 — telemetría ATS → force feedback.");
    println!("   Tasa: ~15 Hz con detección de cambios.");
    println!();
    println!("   Esperando telemetría de ATS...");

    let mut telem = loop {
        match TelemetryReader::open() {
            Ok(r) => break r,
            Err(_) => sleep(Duration::from_secs(2)),
        }
    };
    println!("✓ Telemetría conectada.");

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;
    println!("✓ G923 FFB conectado (feature_idx={}).", ffb.feature_index());
    println!();
    println!("   Maneja en ATS. Ctrl+C para salir.");
    println!();

    let mut spring_slot: Option<u8> = None;
    let mut damper_slot: Option<u8> = None;
    let mut lateral_slot: Option<u8> = None;
    let mut last_spring_coeff: f32 = 0.0;
    let mut last_damper_coeff: f32 = 0.0;
    let mut last_lateral_force: f32 = 0.0;
    let mut smoothed_lateral: f32 = 0.0;
    let mut last_status = Instant::now();
    let mut was_paused = true;

    loop {
        let loop_start = Instant::now();

        if !telem.has_new_frame() {
            sleep(Duration::from_millis(5));
            continue;
        }

        let t = telem.read();

        // ── Pausa: quitar todas las fuerzas ──────────────────────
        if t.paused != 0 {
            if !was_paused {
                if let Some(s) = spring_slot.take() { let _ = ffb.destroy(s); }
                if let Some(s) = damper_slot.take() { let _ = ffb.destroy(s); }
                if let Some(s) = lateral_slot.take() { let _ = ffb.destroy(s); }
                last_spring_coeff = 0.0;
                last_damper_coeff = 0.0;
                last_lateral_force = 0.0;
                smoothed_lateral = 0.0;
                was_paused = true;
                println!("  ⏸ pausa — fuerzas desactivadas");
            }
            sleep(Duration::from_millis(200));
            continue;
        }
        if was_paused {
            println!("  ▶ juego activo — fuerzas activadas");
            was_paused = false;
        }

        let speed_kmh = t.speed * 3.6;

        // ── Spring: solo actualizar si cambió significativamente ──
        let coeff = (SPRING_BASE + speed_kmh * SPRING_PER_KMH).min(SPRING_MAX);

        if (coeff - last_spring_coeff).abs() > SPRING_THRESHOLD || spring_slot.is_none() {
            let new = ffb.upload_spring(coeff as i16, SPRING_SAT);
            if let Ok(new_slot) = new {
                if let Some(old) = spring_slot {
                    let _ = ffb.destroy(old);
                }
                spring_slot = Some(new_slot);
                last_spring_coeff = coeff;
            }
        }

        // ── Damper: anti-oscillation proporcional a velocidad ────
        let damp = (DAMPER_BASE + speed_kmh * DAMPER_PER_KMH).min(DAMPER_MAX);
        if (damp - last_damper_coeff).abs() > DAMPER_THRESHOLD || damper_slot.is_none() {
            let new = ffb.upload_damper(damp as i16, SPRING_SAT);
            if let Ok(new_slot) = new {
                if let Some(old) = damper_slot {
                    let _ = ffb.destroy(old);
                }
                damper_slot = Some(new_slot);
                last_damper_coeff = damp;
            }
        }

        // ── Lateral force: con smoothing exponencial ─────────────
        let raw_lat = (t.accel_x * LATERAL_GAIN).clamp(-LATERAL_MAX, LATERAL_MAX);
        smoothed_lateral = smoothed_lateral * LATERAL_SMOOTHING + raw_lat * (1.0 - LATERAL_SMOOTHING);
        let lat = smoothed_lateral;

        if (lat - last_lateral_force).abs() > LATERAL_THRESHOLD {
            if lat.abs() > LATERAL_DEADZONE {
                let new = ffb.upload_constant(lat as i16, 200);
                if let Ok(new_slot) = new {
                    if let Some(old) = lateral_slot {
                        let _ = ffb.destroy(old);
                    }
                    lateral_slot = Some(new_slot);
                }
            } else if let Some(old) = lateral_slot.take() {
                let _ = ffb.destroy(old);
            }
            last_lateral_force = lat;
        }

        // ── Status cada 3 segundos ──────────────────────────────
        if last_status.elapsed() > Duration::from_secs(3) {
            println!(
                "  {:>6.1} km/h | {:>5.0} rpm | dir {:>+5.1}% | spr {:>5} | dmp {:>5} | lat {:>+6}",
                speed_kmh,
                t.rpm,
                t.steering * 100.0,
                coeff as i16,
                damp as i16,
                lat as i16,
            );
            last_status = Instant::now();
        }

        // ── Rate limit ──────────────────────────────────────────
        let elapsed = loop_start.elapsed();
        if elapsed < LOOP_PERIOD {
            sleep(LOOP_PERIOD - elapsed);
        }
    }
}
