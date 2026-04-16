# g923-mac-ffb

Driver userspace + daemon de Force Feedback para **Logitech G923 Racing Wheel (variante Xbox)** en **macOS Apple Silicon** (M1–M4). Objetivo inicial: FFB funcional en American Truck Simulator / Euro Truck Simulator 2 nativos Mac. Objetivo extendido: juegos de carreras corriendo en el stack Sikarugir (Steam + D3DMetal / GPTK).

## Contexto de una sola pantalla

- **Hardware target**: Logitech G923 "Xbox One and PC", VID `0x046d`, PID `0xc26e` (native mode — macOS lo enumera así sin necesidad de mode switch)
- **Host**: Mac Mini M4, macOS actual (Sonoma/Sequoia/Tahoe)
- **Stack**: Rust + crate `hidapi` (usa IOHIDManager en el backend macOS)
- **Filosofía**: **100% userspace, sin DriverKit, sin kexts, sin entitlements, sin cuenta developer pagada.** Todo vía `IOHIDDeviceSetReport` / `IOHIDDeviceGetReport`.
- **Protocolo**: HID++ 4.2 confirmado vía hello-world del `Root.GetProtocolVersion` (ver `docs/hidpp-protocol.md`)

## Estado actual (lo que funciona)

- ✅ Enumeración de las 4 colecciones HID del G923 (`cargo run --bin g923-enumerate`)
- ✅ Apertura de la colección HID++ long (`usage_page=0xFF43`, `usage=0x0602`) desde userspace
- ✅ Round trip HID++: send `Root.GetProtocolVersion` → receive `HID++ 4.2` + ping echo (`cargo run --bin g923-ping`)
- ✅ Quirk identificado: el G923 Xbox responde en **report ID `0x12`**, no `0x11`. Ya está manejado en el decoder.

## Arquitectura objetivo

Ver `docs/architecture.md`. Resumen:

```
┌──────────────┐   telemetry   ┌─────────────────┐   HID++    ┌───────┐
│ ATS/ETS2     │──────────────▶│ g923 daemon     │───────────▶│ G923  │
│ (SCS Mac)    │   shm / tcp   │ (Rust, userland)│  output    │ Xbox  │
└──────────────┘               └─────────────────┘  reports   └───────┘
       ▲                                │
       │ input (steering/pedales)       │ input reports
       │ via IOHIDManager del sistema   │ vía hidapi
       └────────────────────────────────┘
```

El daemon **no intercepta** la ruta de input del juego — solo calcula FFB a partir de telemetría y lo inyecta al wheel directamente. Los juegos siguen leyendo el joystick como HID genérico (lo que ya hacen).

## Comandos

```bash
cargo build --bins
cargo run --bin g923-enumerate   # lista colecciones HID del wheel
cargo run --bin g923-ping        # HID++ Root.GetProtocolVersion hello world
```

## Lo que NO hacer

- **No DriverKit.** El wheel ya está expuesto como HID por macOS. No necesitamos reclamear el USB interface.
- **No kexts.** Muertos desde Catalina, imposibles en Apple Silicon sin bajar seguridad.
- **No `ForceFeedback.framework`.** Legacy, requería plug-ins por dispositivo que Logitech nunca actualizó para Apple Silicon. Accedemos al wheel directo vía output reports HID.
- **No `dinput8.dll` injection estilo CrossWheel.** De momento no; si llegamos a juegos Windows en Sikarugir que no expongan telemetría, evaluamos ese camino como Fase 5.
- **No intentes abrir el wheel en modo exclusivo** (`kIOHIDOptionsTypeSeizeDevice`) a menos que sea estrictamente necesario — el sistema (WindowServer, GameController framework) mantiene el device abierto en modo compartido y romper eso deshabilita input en el juego.

## Documentación interna

- [`docs/hardware-discovery.md`](docs/hardware-discovery.md) — resultados de `ioreg`, colecciones HID, report descriptors decodificados
- [`docs/hidpp-protocol.md`](docs/hidpp-protocol.md) — framing HID++ 4.2, comandos conocidos, quirks del G923 Xbox
- [`docs/architecture.md`](docs/architecture.md) — daemon, telemetry bridge, integración con juegos
- [`docs/roadmap.md`](docs/roadmap.md) — fases con estado actual
- [`docs/references.md`](docs/references.md) — repos externos clave (`new-lg4ff`, SDL3, FreeTheWheel, kernel Linux)

## Decisiones arquitectónicas ya tomadas

1. **Rust sobre Swift/C** — portabilidad, ecosistema crates, concurrencia segura para el daemon, facilidad de testing cruzado contra Linux `new-lg4ff`.
2. **`hidapi` crate sobre `IOKit` directo** — menos boilerplate, mismo poder, abstracción ya probada. Si alguna vez falla en un edge case específico de macOS, caemos a `IOKit` puntualmente.
3. **Daemon-based, no library** — el daemon mantiene ownership del path HID++, expone API local (Unix socket o TCP) a clientes. Un solo punto de escritura al wheel evita contención.
4. **Telemetry-first, no injection** — leer telemetría del juego es más simple, más estable y más portable que inyectar DLLs o hookear engines. Pierde fidelidad vs FFB nativo del juego, pero para camiones es indistinguible.

## Links rápidos

- Protocolo de referencia: [berarma/new-lg4ff](https://github.com/berarma/new-lg4ff) (Linux, GPL)
- HID++ 4.2 en kernel Linux: [`hid-logitech-hidpp.c`](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c)
- SDL3 FFB PR: [#11598](https://github.com/libsdl-org/SDL/pull/11598)
