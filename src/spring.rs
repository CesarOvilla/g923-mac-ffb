// Fase 2d — spring autocentrado programable.
//
// Prueba pausada de 4 fases: suave, normal, fuerte, sin spring propio.
// El usuario empuja el aro con la mano y siente la resistencia de cada
// configuración, luego lo suelta y ve qué tan rápido (o si acaso) vuelve
// al centro.

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

const SAT_FULL: u16 = 0xFFFF;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de spring (autocentrado) G923 — 4 fases pausadas.");
    println!();
    println!("   En cada fase vas a empujar el aro con la mano hacia un lado,");
    println!("   sentir la resistencia, y soltarlo para ver cómo vuelve al centro.");
    println!();
    println!("   Cada fase espera tu Enter, sin prisa.");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;

    run_phase(
        &ffb,
        "1",
        "Spring SUAVE",
        8_000,
        "resistencia ligera; al soltar vuelve al centro despacio",
    )?;
    run_phase(
        &ffb,
        "2",
        "Spring NORMAL",
        16_000,
        "resistencia media; como un auto normal",
    )?;
    run_phase(
        &ffb,
        "3",
        "Spring FUERTE",
        30_000,
        "resistencia dura; cuesta sacarlo del centro, regreso rápido",
    )?;

    // Fase 4: sin spring propio. Solo queda el spring de fábrica del firmware
    // (el que vive en el slot 0 reservado del G923).
    println!();
    println!("── Fase 4: SIN spring propio (comparación)");
    println!("   esperado  = solo queda el autocentrado de fábrica del G923.");
    println!("               Debe sentirse MÁS SUELTO que la fase 1.");
    wait_for_enter("   [Enter cuando estés listo para sentir el aro libre] ");
    ffb.reset_all()?;
    println!("   ✓ spring propio eliminado");
    wait_for_enter("   [empuja el aro, siente, y pulsa Enter para cerrar] ");

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame qué sentiste en cada fase:");
    println!("  1 (suave):  ¿resistencia ligera al empujar?");
    println!("  2 (normal): ¿claramente más resistente que la 1?");
    println!("  3 (fuerte): ¿claramente más resistente que la 2?");
    println!("  4 (sin propio): ¿más suelto que la 1?");
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
    wait_for_enter("   [Enter para programar este spring] ");

    let slot = ffb.upload_spring(coefficient, SAT_FULL)?;
    println!("   ✓ spring activo en slot {slot}");
    wait_for_enter("   [empuja el aro, suéltalo, siente, y pulsa Enter para la siguiente fase] ");
    ffb.destroy(slot)?;
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
