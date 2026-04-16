# Roadmap

Estado al 2026-04-15.

## Fase 0 — Reconocimiento ✅

- [x] Detectar G923 Xbox en macOS Apple Silicon
- [x] Dump de `system_profiler` y `ioreg` para mapear interfaces y colecciones
- [x] Decodificar report descriptor de interface 0
- [x] Confirmar que macOS lo clasifica como `GameControllerType = 1`
- [x] Verificar que `hidapi` abre la colección HID++ long sin permisos especiales

**Salida**: `docs/hardware-discovery.md`

## Fase 1 — USB mode switch ✅ (saltada)

- [x] Verificar PID actual del wheel
- [x] **Confirmado**: el wheel arranca directamente en PID `0xc26e` (modo nativo). No se requiere mode switch, no se requiere DriverKit.

## Fase 2 — HID++ hello world ✅

- [x] Crear proyecto Rust con `hidapi`
- [x] Binario `g923-enumerate` que lista las 4 colecciones HID
- [x] Binario `g923-ping` que envía `Root.GetProtocolVersion`
- [x] **Confirmado**: wheel responde `HID++ 4.2` con ping echo correcto
- [x] **Descubierto**: quirk del G923 Xbox — responses llegan en report ID `0x12`, no `0x11`

**Salida**: `src/enumerate.rs`, `src/ping.rs`, `docs/hidpp-protocol.md`

## Fase 2b — Feature discovery ✅

- [x] Helper genérico `HidppDevice::send_sync(feature_idx, function, params)` (`src/hidpp.rs`)
  - Drena reports asíncronos del volante hasta que llega la reply que hace echo de `(feature_idx, function|sw_id)`
  - Decodifica error frames (`feature_idx = 0x8F`) y los convierte en `Error::Protocol { code }`
  - Acepta tanto report ID `0x11` como el quirk `0x12` del G923 Xbox
- [x] Cliente IRoot (`HidppDevice`):
  - [x] `get_protocol_version() -> (major, minor, ping_echo)`
  - [x] `get_feature(feature_id) -> FeatureInfo { index, type, version }`
- [x] Cliente IFeatureSet (`FeatureSet<'a>`):
  - [x] `get_count() -> u8`
  - [x] `get_feature_id(index) -> (feature_id, type, version)`
- [x] Binario `g923-discover` que imprime la tabla completa de features + resumen de targets de interés
- [x] **Tabla real del firmware capturada y documentada** en `docs/hidpp-protocol.md`:
  - 21 features + IRoot
  - **ForceFeedback `0x8123` @ índice 11, v1** ← el objetivo
  - Haptics `0x8124` @ índice 14 (hidden/engineering)
  - DeviceFwVersion `0x0003` @ 2, DeviceName `0x0005` @ 3
  - 6 features `0x80xx`/`0x81xx` wheel-specific sin identificar (no bloquean Fase 2c)
  - `LEDControl 0x1300` NO presente — si hay control de LEDs vive en otra feature

**Meta**: una única ejecución que imprime "el G923 soporta estas features y están en estos índices". ✅ Cumplida.

## Fase 2c — Constant force MVP ✅

- [x] Cliente de `ForceFeedback 0x8123` en `src/ffb.rs`:
  - [x] `GetInfo()` — devuelve raw slot count (64 en G923, 63 usables tras restar el slot reservado de fábrica)
  - [x] `ResetAll()` — limpia todos los slots
  - [x] `DownloadEffect(constant, force, duration)` — programa constant force con `EFFECT_AUTOSTART`
  - [x] `Destroy(slot)` — libera slot
  - [x] `GetGlobalGains()` / `SetGlobalGains(gain, boost)` — gain default = 0xFFFF
  - [x] `GetAperture()` / `SetAperture(deg)` — aperture default = 900°
  - [x] `Play(slot)` / `Stop(slot)` — implementados pero documentados como **no-op en G923 Xbox** (ver quirks abajo)
- [x] Binario `g923-constant-force` con 3 fases pausadas (LEFT 50% → RIGHT 50% → LEFT 95%)
- [x] **Milestone físico cumplido**: el volante empuja en ambas direcciones con magnitudes diferenciables, controlado 100% desde userspace HID++ sobre Apple Silicon.

### Quirks del G923 Xbox descubiertos durante 2c

Estos NO están en `hid-logitech-hidpp.c` (que cubre G920) y son específicos al firmware Xbox del G923:

1. **`SetEffectState(PLAY)` (function 3) es silenciado.** El device responde sin error pero nunca arranca el efecto. La única forma de hacer que un constant force se ejecute es con el bit `EFFECT_AUTOSTART = 0x80` en el byte de tipo durante `DownloadEffect`. La lib lo aplica siempre por defecto en `upload_constant`.
2. **Convención de signo invertida.** El kernel Linux/G920 usa `+force = derecha`. El G923 Xbox usa `+force = izquierda` (POV del conductor). La lib niega internamente para que los callers sigan usando convención natural (`+ = derecha`).
3. **Slot 0 reservado.** El device asigna `slot = 1` al primer `DownloadEffect`. Coincide con `HIDPP_FF_RESERVED_SLOTS = 1` del kernel. El slot reservado probablemente aloja el spring de autocentrado de fábrica (visible cuando soltamos un efecto y el aro vuelve solo al centro).
4. **Gain y aperture default ya son útiles.** Al arrancar: `gain = 0xFFFF`, `boost = 0`, `aperture = 900°`. No hace falta tocarlos para FFB básico.

## Fase 2d — FFB engine completo 🟡 en progreso

- [x] Very-long report (`0x12`, 64B) añadido a `HidppDevice::send_sync` con auto-switch long↔very-long por tamaño de params. Necesario porque los condition effects son 18 bytes.
- [x] `upload_condition(effect_type, left/right_coeff, left/right_sat, deadband, center)` en `ffb.rs` — port verbatim del layout `hidpp_ff_upload_effect()` del kernel para el grupo `FF_SPRING..FF_INERTIA`.
- [x] **Spring** (auto-centering programable) — `upload_spring(coefficient, saturation)`, validado físicamente con 4 fases en `g923-spring` (suave/normal/fuerte/sin). Quirk de signo refinado: los coeficientes NO se flipan (son stiffness, no dirección), solo `center` sí. Ver Quirk 2b en `docs/hidpp-protocol.md`.
- [x] **Damper** (resistencia proporcional a velocidad angular) — `upload_damper`, validado físicamente con `g923-damper`. Lento = libre, rápido = resistencia, progresión clara en 3 niveles.
- [x] **Friction** (fricción estática) — `upload_friction`, validado con `g923-friction`. Drag constante sin importar velocidad/posición, 3 niveles progresivos.
- [x] **Inertia** (masa virtual) — `upload_inertia`, validado con `g923-inertia`. Resistencia al arrancar clara; momentum al soltar es sutil (absorbido por drag mecánico del motor/poleas).

### Descubrimiento Fase 2d: el "estado default" del G923

Después de `reset_all()` el wheel NO queda completamente libre. Vuelve al estado default del firmware, que vive en el slot 0 reservado y combina un spring suave de autocentrado con algo de friction/drag mecánico. Esto es visible en la fase 4 de los binarios de test (`g923-spring`, `g923-damper`): el aro sigue teniendo "feel" aunque no haya ningún host effect programado.

No es un bug — es la configuración base del firmware Logitech. No bloquea nada porque en Fase 4 (telemetry loop a 100-500 Hz) nuestros efectos siempre estarán activos y sobrescribirán la base. Mencionar en el README final como característica, no como limitación.
- [ ] Envelopes (attack/sustain/fade) para efectos dinámicos como baches
- [ ] Manager de slots: asignar efectos a los 4 slots HW automáticamente
- [ ] Lock rotation range (90°..900°) — `set_aperture` (fn 6 de `0x8123`) es **silenciado** por el G923 Xbox (acepta sin error pero no cambia el rango físico). El control de rango probablemente vive en una de las features `0x80xx`/`0x81xx` sin identificar. Requiere captura USB de GHub para mapear. No bloquea: default 900° funciona para camiones.

**Salida**: módulo `ffb::engine` expuesto internamente, suite de ejemplos manuales para tunear a mano.

## Fase 3 — Input reader y parser

- [ ] Abrir colección Generic Desktop Joystick (`0x01 / 0x04`)
- [ ] Parsear report ID `0x01` según el descriptor:
  - [ ] Steering angle (16-bit signed)
  - [ ] Throttle, brake, clutch (8-bit cada uno)
  - [ ] Hat switch (4-bit)
  - [ ] 23 botones (bitmap)
- [ ] Exponer stream de eventos para loggear/debug
- [ ] Integrar al daemon como fuente auxiliar (no sustituye al juego como consumer)

## Fase 4 — Telemetry bridge ATS/ETS2

- [ ] Evaluar el SCS Telemetry SDK oficial vs. forks comunitarios (`scs-sdk-plugin`)
- [ ] Compilar el plugin para macOS Apple Silicon si no existe pre-built
- [ ] Implementar lector de shared memory que publica el plugin
- [ ] Mapear telemetría → inputs del FFB engine:
  - Centering force = `f(speed, steering_angle)`
  - Weight = `f(cargo_mass, speed)`
  - Surface bumps = `f(surface_type, speed, wheel_rpm)`
  - Collision jolts = `f(delta acceleration)`
  - Curb feedback = `f(tire position off-road)`
- [ ] Loop del daemon: 100–500 Hz leyendo telemetría, recalculando efectos, actualizando el wheel
- [ ] Perfilar latencia (objetivo <5 ms telemetry → HID output)

**Milestone usable**: correr ATS/ETS2 con FFB procedural sintiendo el camión en vivo.

## Fase 5 — Racing games en Sikarugir

- [ ] Inventario de juegos objetivo: Assetto Corsa, ACC, AMS2, Dirt Rally 2.0, etc.
- [ ] Priorizar por qué exponen telemetría accesible:
  - [ ] AC / ACC shared memory
  - [ ] Dirt Rally UDP telemetry
  - [ ] F1 UDP telemetry
- [ ] Para cada juego con telemetría: ingestor específico en el daemon
- [ ] Para juegos sin telemetría accesible: decidir si construir `dinput8.dll` injection dentro del bottle Sikarugir

## Fase 6 — UX, empaquetado, distribución

- [ ] `launchctl` plist para que el daemon arranque al login
- [ ] CLI `g923` con subcomandos (`start`, `stop`, `status`, `test`, `tune`)
- [ ] Archivo de config TOML (intensidades globales, perfiles por juego)
- [ ] Empaquetado `.pkg` o `.app` firmado y notariado
- [ ] README público, demo video

## Fuera de scope (hoy)

- Otros wheels Logitech (G29, G920, G923 PS). Revisita cuando el G923 Xbox esté pulido.
- Wheels no-Logitech (Fanatec, Moza, Thrustmaster). Cada uno tiene su protocolo propio.
- GUI nativa. CLI por ahora.
- Windows / Linux hosts. Rust + hidapi lo haría factible técnicamente, pero no es prioridad.
