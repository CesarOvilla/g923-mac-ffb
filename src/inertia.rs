// Fase 2d — inertia (masa virtual / inercia rotacional).
//
// Inertia simula que el aro tiene más MASA de la que tiene. Efecto:
//   - Cuesta más ARRANCAR a girarlo (inercia de arranque)
//   - Una vez girando, SIGUE SOLO un poco al soltar (momentum)
//   - Más coeficiente = más "volante de inercia" (flywheel)
//
// Diferencia clave:
//   spring:   jala HACIA el centro
//   damper:   resiste la VELOCIDAD
//   friction: resiste con FUERZA CONSTANTE
//   inertia:  resiste el CAMBIO de velocidad (aceleración angular)

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

const SAT_FULL: u16 = 0xFFFF;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de inertia (masa virtual) G923 — 4 fases pausadas.");
    println!();
    println!("   Para sentir la inercia haz esto en cada fase:");
    println!("   1. Arranca de golpe: intenta girar el aro rápido desde parado.");
    println!("      Debe costar más arrancar que mantener el movimiento.");
    println!("   2. Suelta: gira el aro con fuerza y suéltalo.");
    println!("      Debe seguir girando solo un poco (momentum).");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;

    run_phase(
        &ffb,
        "1",
        "Inertia SUAVE",
        8_000,
        "ligera resistencia al arrancar; un poco de momentum al soltar",
    )?;
    run_phase(
        &ffb,
        "2",
        "Inertia NORMAL",
        16_000,
        "cuesta más arrancar; más momentum al soltar",
    )?;
    run_phase(
        &ffb,
        "3",
        "Inertia FUERTE",
        30_000,
        "cuesta mucho arrancar; al soltar sigue girando notablemente",
    )?;

    println!();
    println!("── Fase 4: SIN inertia (comparación)");
    println!("   esperado  = solo el feel default del firmware.");
    println!("               Arrancar debería costar MENOS que la fase 1.");
    wait_for_enter("   [Enter para quitar nuestra inertia] ");
    ffb.reset_all()?;
    println!("   ✓ inertia eliminada");
    wait_for_enter("   [prueba arrancar y soltar, siente, y pulsa Enter para cerrar] ");

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame:");
    println!("  1 (suave):  ¿ligera resistencia al arrancar, algo de momentum al soltar?");
    println!("  2 (normal): ¿más resistencia al arrancar, más momentum?");
    println!("  3 (fuerte): ¿claramente pesado al arrancar, sigue girando al soltar?");
    println!("  4 (sin):    ¿más fácil arrancar que la fase 1?");
    Ok(())
}

fn run_phase(
    ffb: &ForceFeedback,
    label: &str,
    titulo: &str,
    coefficient: i16,
    esperado: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pct = (coefficient.unsigned_abs() as u32 * 100) / i16::MAX as u32;
    println!();
    println!("── Fase {label}: {titulo}");
    println!("   coeficiente = {coefficient}  ({pct}% de escala completa)");
    println!("   saturación  = 0x{SAT_FULL:04x} (máxima)");
    println!("   esperado    = {esperado}");
    wait_for_enter("   [Enter para programar esta inertia] ");

    let slot = ffb.upload_inertia(coefficient, SAT_FULL)?;
    println!("   ✓ inertia activa en slot {slot}");
    wait_for_enter("   [prueba arrancar y soltar el aro, y pulsa Enter para la siguiente fase] ");
    ffb.destroy(slot)?;
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
