// Fase 2d — envelopes (attack/fade) sobre constant force.
//
// El envelope permite moldear la forma de un pulso de fuerza:
// en vez de un golpe cuadrado (encendido → apagado de golpe),
// podemos hacer rampas de entrada (attack) y salida (fade).
//
// Fase 1: golpe plano (sin envelope) — referencia
// Fase 2: fade out — empieza fuerte, se desvanece suavemente
// Fase 3: attack in — empieza suave, sube hasta la fuerza full
// Fase 4: bache completo — sube rápido, sustain breve, fade largo
//         (simula pasar por un bache en la carretera)

use g923_mac_ffb::ffb::ForceFeedback;
use g923_mac_ffb::hidpp::HidppDevice;
use hidapi::HidApi;
use std::io::{self, BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

const F60: i16 = -20_000; // izquierda 60%

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test de envelopes (attack/fade) G923 — 4 fases.");
    println!();
    println!("   Deja el volante LIBRE. Cada fase manda un pulso a la izquierda.");
    println!("   Lo que cambia es la FORMA del pulso:");
    println!("     plano  = empieza fuerte, termina fuerte (cuadrado)");
    println!("     fade   = empieza fuerte, se desvanece suavemente");
    println!("     attack = empieza suave, sube hasta full");
    println!("     bache  = sube rápido, golpe breve, desvanece lento");
    println!();

    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;
    let ffb = ForceFeedback::new(&dev)?;
    ffb.reset_all()?;

    // Fase 1: plano (sin envelope)
    println!("── Fase 1: PLANO (sin envelope)");
    println!("   esperado = golpe seco a la izquierda, empieza y termina de golpe");
    wait_for_enter("   [Enter para disparar] ");
    sleep(Duration::from_secs(1));
    let slot = ffb.upload_constant(F60, 1500)?;
    sleep(Duration::from_millis(1500));
    ffb.destroy(slot)?;
    println!("   ✓ pulso enviado");
    wait_for_enter("   [Enter para la siguiente fase] ");

    // Fase 2: fade out
    println!();
    println!("── Fase 2: FADE OUT");
    println!("   esperado = empieza fuerte, se DESVANECE suavemente durante 1.2s");
    wait_for_enter("   [Enter para disparar] ");
    sleep(Duration::from_secs(1));
    let slot = ffb.upload_constant_envelope(
        F60,
        1500,
        255,  // attack: empieza a fuerza full (sin rampa de subida)
        0,    // attack length: instantáneo
        0,    // fade: baja hasta fuerza cero
        1200, // fade length: 1.2 segundos de desvanecimiento
    )?;
    sleep(Duration::from_millis(1500));
    ffb.destroy(slot)?;
    println!("   ✓ pulso enviado");
    wait_for_enter("   [Enter para la siguiente fase] ");

    // Fase 3: attack in
    println!();
    println!("── Fase 3: ATTACK IN");
    println!("   esperado = empieza SUAVE, SUBE gradualmente durante 1s hasta fuerza full");
    wait_for_enter("   [Enter para disparar] ");
    sleep(Duration::from_secs(1));
    let slot = ffb.upload_constant_envelope(
        F60,
        1500,
        0,    // attack: empieza desde cero
        1000, // attack length: sube durante 1 segundo
        255,  // fade: termina a fuerza full (sin desvanecimiento)
        0,    // fade length: instantáneo
    )?;
    sleep(Duration::from_millis(1500));
    ffb.destroy(slot)?;
    println!("   ✓ pulso enviado");
    wait_for_enter("   [Enter para la siguiente fase] ");

    // Fase 4: bache (attack rápido + fade largo)
    println!();
    println!("── Fase 4: BACHE (attack rápido + sustain + fade largo)");
    println!("   esperado = golpe RÁPIDO (200ms), mantiene BREVE, se DESVANECE lento (800ms)");
    println!("              simula pasar por un bache o tope en la carretera");
    wait_for_enter("   [Enter para disparar] ");
    sleep(Duration::from_secs(1));
    let slot = ffb.upload_constant_envelope(
        F60,
        1500,
        0,   // attack: empieza desde cero
        200, // attack: sube rápido (200ms)
        0,   // fade: baja hasta cero
        800, // fade: desvanece en 800ms
    )?;
    sleep(Duration::from_millis(1500));
    ffb.destroy(slot)?;
    println!("   ✓ pulso enviado");

    ffb.reset_all()?;

    println!();
    println!("──────────────────────────────────────────────────────");
    println!("listo. Cuéntame para cada fase:");
    println!("  1 (plano):  ¿golpe seco, empieza y para de golpe?");
    println!("  2 (fade):   ¿empieza fuerte, se desvanece?");
    println!("  3 (attack): ¿empieza suave, sube?");
    println!("  4 (bache):  ¿sube rápido, breve, y se desvanece?");
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
