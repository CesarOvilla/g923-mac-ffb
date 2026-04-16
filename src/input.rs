// Fase 3 — visor de input del G923 en tiempo real.
//
// Mapa de bytes del report ID 0x01 (11 bytes), confirmado empíricamente:
//
//   [0]    report ID (0x01)
//   [1]    [7:4] botones primeros 4 | [3:0] hat switch (0-7 dirs, 8 neutro)
//   [2-4]  botones restantes (bits 4-22)
//   [5-6]  steering 16-bit LE (0x0000=full left, 0xFFFF=full right)
//   [7]    acelerador (invertido: 0xFF=suelto, 0x00=a fondo)
//   [8]    freno (invertido)
//   [9]    clutch (invertido)
//   [10]   constante vendor (0x05)

use g923_mac_ffb::hidpp::{G923_XBOX_PID, LOGITECH_VID};
use hidapi::HidApi;
use std::io::{self, Write};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Visor de input G923 — tiempo real.");
    println!("   Mueve el volante, pedales y botones. Ctrl+C para salir.");
    println!();

    let api = HidApi::new()?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == LOGITECH_VID
                && d.product_id() == G923_XBOX_PID
                && d.usage_page() == 0x01
                && d.usage() == 0x04
        })
        .ok_or("colección Joystick no encontrada")?;

    let dev = info.open_device(&api)?;

    // limpiar pantalla
    print!("\x1b[2J\x1b[H");
    io::stdout().flush().ok();

    let mut buf = [0u8; 64];
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    let mut other_report = String::from("(ninguno aún)");

    loop {
        let n = dev.read_timeout(&mut buf, 100)?;
        if n == 0 {
            continue;
        }
        // capturar reports que NO son 0x01 (dial, vendor, etc.)
        if n < 11 || buf[0] != 0x01 {
            other_report = format!("{} bytes → {}", n, hex(&buf[..n]));
            continue;
        }

        let now = Instant::now();
        if now.duration_since(last_draw) < Duration::from_millis(50) {
            continue;
        }
        last_draw = now;

        let hat = buf[1] & 0x0F;
        // empaquetado contiguo: nibble alto de buf[1] + buf[2] + buf[3] + buf[4]
        let buttons = ((buf[1] as u32 >> 4) & 0x0F)
            | ((buf[2] as u32) << 4)
            | ((buf[3] as u32) << 12)
            | ((buf[4] as u32) << 20);
        let steering = u16::from_le_bytes([buf[5], buf[6]]);
        let throttle = 255 - buf[7];
        let brake = 255 - buf[8];
        let clutch = 255 - buf[9];

        let steer_pct = ((steering as f32 / 65535.0) * 200.0 - 100.0) as i32;
        let thr_pct = (throttle as f32 / 255.0 * 100.0) as u32;
        let brk_pct = (brake as f32 / 255.0 * 100.0) as u32;
        let clt_pct = (clutch as f32 / 255.0 * 100.0) as u32;

        // mover cursor al inicio
        print!("\x1b[H");

        println!("  ╔══════════════════════════════════════════════╗");
        println!("  ║          G923 INPUT — TIEMPO REAL            ║");
        println!("  ╠══════════════════════════════════════════════╣");
        println!("  ║                                              ║");
        println!(
            "  ║  Volante:  {:>+4}%  {}  ║",
            steer_pct,
            steering_bar(steer_pct),
        );
        println!("  ║                                              ║");
        println!(
            "  ║  Aceler:   {:>3}%   {}  ║",
            thr_pct,
            pedal_bar(thr_pct),
        );
        println!(
            "  ║  Freno:    {:>3}%   {}  ║",
            brk_pct,
            pedal_bar(brk_pct),
        );
        println!(
            "  ║  Clutch:   {:>3}%   {}  ║",
            clt_pct,
            pedal_bar(clt_pct),
        );
        println!("  ║                                              ║");
        println!(
            "  ║  Hat:      {:<8}                           ║",
            hat_name(hat),
        );
        println!(
            "  ║  Botones:  {:<34} ║",
            button_names(buttons),
        );
        println!("  ║                                              ║");
        println!(
            "  ║  Raw:  {}  ║",
            hex(&buf[..11]),
        );
        println!("  ║                                              ║");
        println!(
            "  ║  Otro:  {:<37} ║",
            if other_report.len() > 37 { &other_report[..37] } else { &other_report },
        );
        println!("  ╠══════════════════════════════════════════════╣");
        println!("  ║  Ctrl+C para salir                           ║");
        println!("  ╚══════════════════════════════════════════════╝");

        io::stdout().flush().ok();
    }
}

fn steering_bar(pct: i32) -> String {
    let width = 25;
    let center = width / 2;
    let mut bar = vec![' '; width];
    bar[center] = '│';

    let pos = ((pct + 100) as usize * (width - 1)) / 200;
    let pos = pos.min(width - 1);
    bar[pos] = '●';

    format!("[{}]", bar.iter().collect::<String>())
}

fn pedal_bar(pct: u32) -> String {
    let filled = (pct as usize * 20) / 100;
    let empty = 20 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn hat_name(hat: u8) -> &'static str {
    match hat {
        0 => "↑ arriba",
        1 => "↗ arr-der",
        2 => "→ derecha",
        3 => "↘ abj-der",
        4 => "↓ abajo",
        5 => "↙ abj-izq",
        6 => "← izquier",
        7 => "↖ arr-izq",
        _ => "· neutro",
    }
}

fn button_names(bits: u32) -> String {
    // Mapa confirmado empíricamente contra el G923 Xbox.
    // buf[1] upper nibble → bits 0-3
    // buf[2]              → bits 4-11
    // buf[3]              → bits 12-19
    // buf[4]              → bits 20-22+
    let names = [
        "A", "B", "X", "Y",
        "Shift▸", "Shift◂", "☰Menú", "⧉Vista", "RSB", "LSB", "Xbox", "?11",
        "?12", "?13", "?14", "?15", "?16", "?17", "Dial+", "Dial−",
        "Dial▸", "Dial◂", "Enter",
    ];
    let mut pressed = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if bits & (1 << i) != 0 {
            pressed.push(*name);
        }
    }
    if pressed.is_empty() {
        "(ninguno)".into()
    } else {
        pressed.join(" ")
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
