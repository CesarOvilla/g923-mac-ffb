// Test: ¿el G923 Xbox soporta el protocolo clásico de Logitech FFB?
//
// El protocolo clásico (usado por G29/G920/G923 PS) manda comandos
// de 7 bytes directamente a la colección Joystick, no vía HID++.
// Si el G923 Xbox lo soporta, podemos usar el mismo enfoque que
// fffb y resolver el problema de coexistencia con ATS.
//
// Patrón de fffb: seize → write → close (para cada comando).

use g923_mac_ffb::hidpp::{G923_XBOX_PID, LOGITECH_VID};
use hidapi::HidApi;
use std::io::{self, BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de protocolo clásico Logitech FFB en G923 Xbox.");
    println!();
    println!("   Vamos a mandar los mismos comandos que usa fffb");
    println!("   (protocolo G29/G920) y ver si el G923 Xbox responde.");
    println!();

    let api = HidApi::new()?;

    // Buscar la colección Joystick (0x01/0x04) — la misma que usa fffb
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == LOGITECH_VID
                && d.product_id() == G923_XBOX_PID
                && d.usage_page() == 0x01
                && d.usage() == 0x04
        })
        .ok_or("Colección Joystick no encontrada")?;

    println!("✓ Colección Joystick encontrada (usage_page=0x01, usage=0x04)");
    println!();

    // ── Test 1: Init clásico ──────────────────────────────────────
    println!("══ TEST 1: Comando init clásico (extended cmd) ══");
    println!("   Manda: [0x30, 0xf8, 0x09, 0x05, 0x01, 0x01, 0x00, 0x00]");
    println!("   Este comando habilita el FFB clásico en G29/G920.");
    wait_for_enter("   [Enter para mandar] ");

    {
        let dev = info.open_device(&api)?;
        let cmd: [u8; 8] = [0x30, 0xf8, 0x09, 0x05, 0x01, 0x01, 0x00, 0x00];
        match dev.write(&cmd) {
            Ok(n) => println!("   ✓ write OK ({n} bytes)"),
            Err(e) => println!("   ✗ write falló: {e}"),
        }
    }

    println!();

    // ── Test 2: Constant force clásico ────────────────────────────
    println!("══ TEST 2: Constant force clásico (slot 0, 80%) ══");
    println!("   Formato clásico: [slot<<4, 0x00, amp, amp, amp, amp, 0x00]");
    println!("   Si el G923 Xbox lo soporta, el volante debería empujar.");
    wait_for_enter("   [Enter para mandar — agarra el volante] ");

    {
        let dev = info.open_device(&api)?;
        // Slot 0 play
        let play: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        // Download constant force: slot 0, type constant (0x00), amplitude ~80%
        let amp: u8 = 0xCC; // ~80% de 0xFF
        let constant: [u8; 8] = [0x00, 0x00, amp, amp, amp, amp, 0x00, 0x00];
        match dev.write(&constant) {
            Ok(n) => println!("   ✓ constant force write OK ({n} bytes)"),
            Err(e) => println!("   ✗ constant force write falló: {e}"),
        }

        // Play effect slot 0
        let play_cmd: [u8; 8] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        match dev.write(&play_cmd) {
            Ok(n) => println!("   ✓ play write OK ({n} bytes)"),
            Err(e) => println!("   ✗ play write falló: {e}"),
        }
    }

    println!("   → ¿Sentiste algo en el volante? (10 segundos)");
    sleep(Duration::from_secs(10));
    wait_for_enter("   [Enter] ");

    // ── Test 3: Autocenter disable clásico ────────────────────────
    println!();
    println!("══ TEST 3: Disable autocenter clásico ══");
    println!("   Si funciona, el volante debería quedar más suelto.");
    wait_for_enter("   [Enter para mandar] ");

    {
        let dev = info.open_device(&api)?;
        let disable_ac: [u8; 8] = [0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        match dev.write(&disable_ac) {
            Ok(n) => println!("   ✓ disable autocenter write OK ({n} bytes)"),
            Err(e) => println!("   ✗ disable autocenter write falló: {e}"),
        }
    }

    println!("   → ¿El volante se siente más suelto? (10 segundos)");
    sleep(Duration::from_secs(10));
    wait_for_enter("   [Enter] ");

    // ── Test 4: HID++ via Joystick con seize pattern ──────────────
    println!();
    println!("══ TEST 4: HID++ via colección Joystick ══");
    println!("   Mismo comando HID++ que funciona vía hidapi,");
    println!("   pero mandado a la colección Joystick.");
    wait_for_enter("   [Enter para mandar spring HID++] ");

    {
        let dev = info.open_device(&api)?;
        // HID++ long: ResetAll FFB
        let reset: [u8; 20] = {
            let mut r = [0u8; 20];
            r[0] = 0x11; // report ID
            r[1] = 0xFF; // device index
            r[2] = 0x0B; // feature index (FFB)
            r[3] = 0x11; // fn=1 (ResetAll) << 4 | sw_id=1
            r
        };
        match dev.write(&reset) {
            Ok(n) => println!("   ✓ HID++ ResetAll write OK ({n} bytes)"),
            Err(e) => println!("   ✗ HID++ ResetAll write falló: {e}"),
        }

        // HID++ spring via very long report
        let mut spring = [0u8; 64];
        spring[0] = 0x12; // report ID (very long)
        spring[1] = 0xFF;
        spring[2] = 0x0B;
        spring[3] = 0x21; // fn=2 (download) << 4 | sw_id=1
        // params
        spring[4 + 1] = 0x06 | 0x80; // SPRING | AUTOSTART
        spring[4 + 8] = 0x3A;  // left_coeff high (15000)
        spring[4 + 9] = 0x98;
        spring[4 + 14] = 0x3A; // right_coeff
        spring[4 + 15] = 0x98;
        spring[4 + 6] = 0x7F;  // left_sat
        spring[4 + 7] = 0xFF;
        spring[4 + 16] = 0x7F; // right_sat
        spring[4 + 17] = 0xFF;
        match dev.write(&spring) {
            Ok(n) => println!("   ✓ HID++ Spring write OK ({n} bytes)"),
            Err(e) => println!("   ✗ HID++ Spring write falló: {e}"),
        }
    }

    println!("   → ¿Sientes resistencia al girar? (10 segundos)");
    sleep(Duration::from_secs(10));

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("Cuéntame para cada test:");
    println!("  1 (init clásico): ¿write OK o error?");
    println!("  2 (constant clásico): ¿sentiste empujón?");
    println!("  3 (disable autocenter): ¿volante más suelto?");
    println!("  4 (HID++ via joystick): ¿sentiste spring?");
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
