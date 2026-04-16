// Fase 2d — friction (fricción estática / drag constante).
//
// Friction opone una resistencia CONSTANTE al movimiento sin importar
// la velocidad ni la posición. Es como girar el volante con grasa
// espesa en la columna: siempre se siente pesado por igual, sin importar
// si giras lento o rápido, cerca o lejos del centro.
//
// Diferencia clave vs los otros:
//   spring:   resistencia ∝ posición (cuanto más lejos del centro, más duro)
//   damper:   resistencia ∝ velocidad (cuanto más rápido, más duro)
//   friction: resistencia constante (siempre igual, sin importar velocidad/posición)

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

const SAT_FULL: u16 = 0xFFFF;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de friction (drag constante) G923 — 4 fases pausadas.");
    println!();
    println!("   La friction se siente como PESO CONSTANTE al girar.");
    println!("   NO depende de la velocidad (eso sería damper) ni de");
    println!("   la posición (eso sería spring).");
    println!();
    println!("   Para sentir la diferencia: gira lento y rápido en cada fase.");
    println!("   Ambos deberían sentirse igual de pesados (esa es la friction).");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;

    run_phase(
        &ffb,
        "1",
        "Friction SUAVE",
        8_000,
        "resistencia ligera constante; lento y rápido se sienten igual",
    )?;
    run_phase(
        &ffb,
        "2",
        "Friction NORMAL",
        16_000,
        "más pesado que fase 1, pero siempre constante",
    )?;
    run_phase(
        &ffb,
        "3",
        "Friction FUERTE",
        30_000,
        "claramente pesado; como girar con grasa espesa en la columna",
    )?;

    println!();
    println!("── Fase 4: SIN friction (comparación)");
    println!("   esperado  = solo queda el feel del firmware base.");
    println!("               Debe sentirse MÁS LIGERO que la fase 1.");
    wait_for_enter("   [Enter para quitar nuestra friction] ");
    ffb.reset_all()?;
    println!("   ✓ friction eliminada");
    wait_for_enter("   [gira el aro, siente, y pulsa Enter para cerrar] ");

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame:");
    println!("  1 (suave):  ¿resistencia ligera constante?");
    println!("  2 (normal): ¿más pesado que la 1?");
    println!("  3 (fuerte): ¿más pesado que la 2?");
    println!("  4 (sin):    ¿más ligero que la 1?");
    println!();
    println!("Clave friction vs damper:");
    println!("  - Friction: girar lento = igual de pesado que girar rápido.");
    println!("  - Damper:   girar lento = fácil, girar rápido = pesado.");
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
    wait_for_enter("   [Enter para programar esta friction] ");

    let slot = ffb.upload_friction(coefficient, SAT_FULL)?;
    println!("   ✓ friction activa en slot {slot}");
    wait_for_enter("   [gira el aro, siente, y pulsa Enter para la siguiente fase] ");
    ffb.destroy(slot)?;
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
