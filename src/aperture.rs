// Fase 2d — aperture / lock rotation range.
//
// El G923 permite restringir su rango de giro por software via
// SetAperture (función 6 de la feature 0x8123). Rango de 180° a 900°.
// Es lo que los juegos usan para cambiar entre "volante de F1" (360°)
// y "volante de camión" (900°).
//
// La restricción es física: el motor endurece los extremos como si
// los topes mecánicos se movieran. No es solo software.

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de aperture (rango de rotación) G923 — 4 fases.");
    println!();
    println!("   En cada fase el rango de giro del volante cambia.");
    println!("   Gira el aro hasta el tope para sentir dónde te detiene.");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;

    let current = ffb.get_aperture()?;
    println!("✓ Apertura actual: {current}°");
    println!();

    run_phase(
        &ffb,
        "1",
        "180°  (kart / go-kart)",
        180,
        "el aro solo gira ±90° desde el centro. Muy corto.",
    )?;
    run_phase(
        &ffb,
        "2",
        "360°  (F1 / formula)",
        360,
        "gira ±180°. Típico de un coche de carreras.",
    )?;
    run_phase(
        &ffb,
        "3",
        "540°  (auto deportivo)",
        540,
        "gira ±270°. Más rango, como un auto de calle.",
    )?;
    run_phase(
        &ffb,
        "4",
        "900°  (camión / auto normal — máximo del G923)",
        900,
        "giro completo ±450°. Rango máximo, vuelta y media por lado.",
    )?;

    // restaurar al máximo
    ffb.set_aperture(900)?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Apertura restaurada a 900°.");
    println!();
    println!("Cuéntame:");
    println!("  1 (180°): ¿tope muy corto, casi no gira?");
    println!("  2 (360°): ¿más rango, claramente mayor que la 1?");
    println!("  3 (540°): ¿más que la 2?");
    println!("  4 (900°): ¿giro máximo, vuelta y media por lado?");
    Ok(())
}

fn run_phase(
    ffb: &ForceFeedback,
    label: &str,
    titulo: &str,
    degrees: u16,
    esperado: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("── Fase {label}: {titulo}");
    println!("   esperado  = {esperado}");
    wait_for_enter("   [Enter para cambiar la apertura] ");

    ffb.set_aperture(degrees)?;
    let readback = ffb.get_aperture()?;
    println!("   ✓ apertura seteada a {degrees}° (readback: {readback}°)");
    wait_for_enter("   [gira el aro hasta el tope, siente, y Enter para la siguiente] ");
    println!();
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
