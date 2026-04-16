// Fase 2c — constant force MVP, pausado 3 fases.
//
// Las tres fases usan el único camino de playback que funciona en
// G923 Xbox: DownloadEffect con el bit EFFECT_AUTOSTART. El signo de
// la fuerza usa la convención natural Linux (+ = derecha, - = izquierda);
// la lib (upload_constant) invierte internamente para el encoding del
// wire del G923 Xbox.

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

const F50: i16 = 16_000;
const F95: i16 = 31_000;
const HOLD_MS: u64 = 1_500;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de constant force G923 — pausado, 3 fases.");
    println!();
    println!("   Deja el volante LIBRE — va a girar solo.");
    println!("   Aleja tazas / cables / manos del aro.");
    println!();
    println!("   Cada fase espera tu Enter para que observes con calma.");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;

    run_phase(
        &ffb,
        "1",
        "IZQUIERDA ←  50%",
        "el volante gira a la IZQUIERDA (POV conductor) por ~1.5s, luego queda libre",
        -F50,
    )?;
    run_phase(
        &ffb,
        "2",
        "DERECHA →  50%",
        "gira a la DERECHA por ~1.5s, espejo de la fase 1",
        F50,
    )?;
    run_phase(
        &ffb,
        "3",
        "IZQUIERDA ←  95%  (casi máximo)",
        "gira a la IZQUIERDA claramente MÁS FUERTE y MÁS RÁPIDO que la fase 1",
        -F95,
    )?;

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame qué pasó en cada fase:");
    println!("  1: ¿IZQUIERDA firme?");
    println!("  2: ¿DERECHA firme?");
    println!("  3: ¿IZQUIERDA claramente MÁS FUERTE que la fase 1?");
    Ok(())
}

fn run_phase(
    ffb: &ForceFeedback,
    label: &str,
    titulo: &str,
    esperado: &str,
    force: i16,
) -> Result<(), Box<dyn std::error::Error>> {
    let pct = (force.unsigned_abs() as u32 * 100) / i16::MAX as u32;
    println!();
    println!("── Fase {label}: {titulo}");
    println!("   fuerza    = {force:+}  ({pct}% de escala completa)");
    println!("   esperado  = {esperado}");
    wait_for_enter("   [Enter cuando estés observando el volante] ");
    println!("   disparando en 2 segundos...");
    sleep(Duration::from_secs(2));

    let dur_ms = (HOLD_MS + 200) as u16;
    let slot = ffb.upload_constant(force, dur_ms)?;
    sleep(Duration::from_millis(HOLD_MS));
    ffb.destroy(slot)?;

    println!("   ✓ pulso enviado");
    wait_for_enter("   [Enter para la siguiente fase] ");
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
