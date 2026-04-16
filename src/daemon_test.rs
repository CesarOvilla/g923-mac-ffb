// Test mínimo: ¿abrir la colección HID++ rompe el input de ATS?
//
// Paso 1: solo abrir (sin comandos) — 15 segundos
// Paso 2: mandar UN spring — 15 segundos
// Paso 3: cerrar
//
// El usuario observa si ATS pierde input en cada paso.

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de compatibilidad daemon + ATS.");
    println!();
    println!("   Abre ATS y verifica que el volante funciona en el juego.");
    println!();

    // ── Paso 1: solo abrir la colección HID++ ────────────────────
    println!("══ PASO 1: Abrir colección HID++ (sin enviar nada) ══");
    wait_for_enter("   [Enter para abrir la conexión HID++] ");

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    println!("   ✓ Colección HID++ abierta.");
    println!("   → Prueba el volante en ATS AHORA. Si no funciona, ESPERA.");
    println!("   → A veces macOS necesita ~15-30 segundos para re-enumerar.");
    println!("   → Mueve el volante cada 5 segundos para ver si vuelve.");
    sleep(Duration::from_secs(30));
    println!("   (30 segundos pasaron)");
    wait_for_enter("   ¿Funcionó el volante en algún momento? [Enter] ");

    // ── Paso 2: mandar un solo comando (get_protocol_version) ────
    println!();
    println!("══ PASO 2: Enviar UN comando HID++ (protocol version) ══");
    wait_for_enter("   [Enter para enviar] ");

    let (major, minor, _) = dev.get_protocol_version()?;
    println!("   ✓ HID++ {major}.{minor} — comando enviado y respuesta recibida.");
    println!("   → ¿Sigue funcionando el volante en ATS? (15 segundos)");
    sleep(Duration::from_secs(15));
    wait_for_enter("   [Enter cuando hayas verificado] ");

    // ── Paso 3: crear un efecto FFB ──────────────────────────────
    println!();
    println!("══ PASO 3: Programar un spring suave ══");
    wait_for_enter("   [Enter para programar el spring] ");

    let ffb = ForceFeedback::new(&dev)?;
    let slot = ffb.upload_spring(5000, 0xFFFF)?;
    println!("   ✓ Spring en slot {slot}.");
    println!("   → ¿Sigue funcionando el volante en ATS?");
    println!("   → ¿Sientes el spring (resistencia al girar)?");
    println!("   (15 segundos para probar)");
    sleep(Duration::from_secs(15));
    wait_for_enter("   [Enter cuando hayas verificado] ");

    // ── Paso 4: destruir y cerrar ────────────────────────────────
    println!();
    println!("══ PASO 4: Limpieza ══");
    ffb.destroy(slot)?;
    ffb.reset_all()?;
    println!("   ✓ Efectos limpiados.");

    println!();
    println!("Cuéntame qué pasó en cada paso:");
    println!("  1: ¿ATS dejó de responder al abrir HID++?");
    println!("  2: ¿Dejó de responder al enviar el comando?");
    println!("  3: ¿Dejó de responder al programar el spring?");
    println!("  Si todo siguió funcionando, el problema es la tasa de comandos.");
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
