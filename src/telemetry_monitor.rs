// Fase 4 — monitor de telemetría ATS en tiempo real.
//
// Lee la shared memory /g923_telemetry publicada por el plugin
// g923_telemetry.dylib que corre dentro de ATS, y muestra los
// valores en un dashboard que se actualiza cada frame.
//
// Útil para verificar que el puente game→plugin→shm funciona
// antes de conectar el FFB loop.

use g923_mac_ffb::telemetry::TelemetryReader;
use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Monitor de telemetría ATS/ETS2 — tiempo real.");
    println!();
    println!("   Abre ATS y empieza a manejar. Ctrl+C para salir.");
    println!("   Esperando shared memory /g923_telemetry...");
    println!();

    let mut reader = loop {
        match TelemetryReader::open() {
            Ok(r) => {
                println!("✓ Shared memory abierta.");
                break r;
            }
            Err(_) => {
                sleep(Duration::from_secs(2));
            }
        }
    };

    print!("\x1b[2J\x1b[H");
    io::stdout().flush().ok();

    loop {
        if !reader.has_new_frame() {
            sleep(Duration::from_millis(5));
            continue;
        }

        let t = reader.read();

        let speed_kmh = t.speed * 3.6;
        let steer_pct = (t.steering * 100.0) as i32;

        print!("\x1b[H");
        println!("  ╔══════════════════════════════════════════════════╗");
        println!("  ║        ATS TELEMETRÍA — TIEMPO REAL             ║");
        println!("  ╠══════════════════════════════════════════════════╣");
        println!("  ║                                                  ║");
        println!(
            "  ║  Velocidad:  {:>6.1} km/h                        ║",
            speed_kmh
        );
        println!(
            "  ║  RPM:        {:>6.0}                              ║",
            t.rpm
        );
        println!(
            "  ║  Dirección:  {:>+4}%  {:25}  ║",
            steer_pct,
            steer_bar(steer_pct)
        );
        println!("  ║                                                  ║");
        println!(
            "  ║  Acelerador: {:>3}%   {:20}       ║",
            (t.throttle * 100.0) as u32,
            pedal_bar((t.throttle * 100.0) as u32)
        );
        println!(
            "  ║  Freno:      {:>3}%   {:20}       ║",
            (t.brake * 100.0) as u32,
            pedal_bar((t.brake * 100.0) as u32)
        );
        println!(
            "  ║  Clutch:     {:>3}%   {:20}       ║",
            (t.clutch * 100.0) as u32,
            pedal_bar((t.clutch * 100.0) as u32)
        );
        println!("  ║                                                  ║");
        println!(
            "  ║  G-lateral:  {:>+6.2}g                            ║",
            t.accel_x
        );
        println!(
            "  ║  G-longit:   {:>+6.2}g                            ║",
            t.accel_z
        );
        println!(
            "  ║  Carga:      {:>6.0} kg                           ║",
            t.cargo_mass
        );
        println!(
            "  ║  Frame:      {:>10}  {}                 ║",
            t.frame,
            if t.paused != 0 { "⏸ PAUSA" } else { "▶ JUEGO" }
        );
        println!("  ║                                                  ║");
        println!(
            "  ║  Susp:  [{:>+5.2} {:>+5.2} {:>+5.2} {:>+5.2}]           ║",
            t.susp_deflection[0],
            t.susp_deflection[1],
            t.susp_deflection[2],
            t.susp_deflection[3]
        );
        println!("  ╠══════════════════════════════════════════════════╣");
        println!("  ║  Ctrl+C para salir                               ║");
        println!("  ╚══════════════════════════════════════════════════╝");

        io::stdout().flush().ok();
        sleep(Duration::from_millis(50));
    }
}

fn steer_bar(pct: i32) -> String {
    let width = 25;
    let center = width / 2;
    let mut bar = vec![' '; width];
    bar[center] = '│';
    let pos = ((pct + 100).max(0).min(200) as usize * (width - 1)) / 200;
    bar[pos] = '●';
    format!("[{}]", bar.iter().collect::<String>())
}

fn pedal_bar(pct: u32) -> String {
    let filled = (pct.min(100) as usize * 20) / 100;
    let empty = 20 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
