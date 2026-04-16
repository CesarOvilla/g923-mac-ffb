// Fase 2d — damper (resistencia proporcional a velocidad angular).
//
// A diferencia del spring (resistencia proporcional al desplazamiento),
// el damper responde a qué tan RÁPIDO giras el aro. Girarlo lento =
// casi sin resistencia. Girarlo rápido = resistencia fuerte. Es la
// sensación "viscosa" de un coche a velocidad, o de manejar con el
// freno de mano puesto.

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

const SAT_FULL: u16 = 0xFFFF;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de damper (resistencia viscosa) G923 — 4 fases pausadas.");
    println!();
    println!("   IMPORTANTE: el damper responde a VELOCIDAD, no a desplazamiento.");
    println!("   Para sentirlo, gira el aro rápido de un lado a otro, no lo dejes quieto.");
    println!("   Compara girar LENTO vs girar RÁPIDO en cada fase.");
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
        "Damper SUAVE",
        8_000,
        "al girar rápido sientes resistencia ligera; al girar lento casi nada",
    )?;
    run_phase(
        &ffb,
        "2",
        "Damper NORMAL",
        16_000,
        "girar rápido se siente viscoso, claramente más duro que fase 1",
    )?;
    run_phase(
        &ffb,
        "3",
        "Damper FUERTE",
        30_000,
        "girar rápido cuesta trabajo; girar lento sigue siendo fácil (ese es el punto)",
    )?;

    println!();
    println!("── Fase 4: SIN damper (comparación)");
    println!("   esperado  = el aro gira libremente sin resistencia viscosa.");
    println!("               No deberías sentir diferencia entre girar lento o rápido.");
    wait_for_enter("   [Enter cuando estés listo para sentir el aro libre] ");
    ffb.reset_all()?;
    println!("   ✓ damper propio eliminado");
    wait_for_enter("   [gira el aro de lado a lado, siente, y pulsa Enter para cerrar] ");

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame qué sentiste en cada fase:");
    println!("  1 (suave):  al girar rápido, ¿resistencia ligera?");
    println!("  2 (normal): al girar rápido, ¿claramente más viscoso que la 1?");
    println!("  3 (fuerte): al girar rápido, ¿claramente más viscoso que la 2?");
    println!("  4 (sin):    ¿el aro gira libre sin resistencia extra?");
    println!();
    println!("Clave para verificar que es DAMPER y no otra cosa:");
    println!("  - Girar LENTO en cualquier fase debe sentirse parecido a la fase 4.");
    println!("  - La diferencia 1→2→3 solo debe aparecer al girar RÁPIDO.");
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
    wait_for_enter("   [Enter para programar este damper] ");

    let slot = ffb.upload_damper(coefficient, SAT_FULL)?;
    println!("   ✓ damper activo en slot {slot}");
    wait_for_enter("   [gira el aro rápido y lento, siente, y pulsa Enter para la siguiente fase] ");
    ffb.destroy(slot)?;
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
