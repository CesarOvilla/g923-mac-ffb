# Arquitectura

## Principio rector

**El driver no se mete en el camino del juego.** Los juegos leen el wheel como HID joystick genérico a través del stack de macOS (`IOHIDManager`, `GCController`, `SDL2/3`, etc.) — eso ya funciona sin nosotros. Nuestro daemon solo se encarga del **Force Feedback**, que es la parte que macOS + Logitech dejaron rota.

Esto evita entero el problema de "exclusivo vs compartido", "interceptar dinput8.dll", "hookear engines propietarios", y cualquier otro camino invasivo. El daemon es un ciudadano cooperativo más del sistema.

## Diagrama

```
┌──────────────────────┐       ┌─────────────────────────┐       ┌──────────┐
│  ATS / ETS2 (SCS)    │       │  g923 daemon            │       │   G923   │
│  nativo macOS        │       │  Rust, userland         │       │   Xbox   │
│                      │       │                         │       │          │
│  ┌───────────────┐   │       │  ┌───────────────┐      │ HID++ │          │
│  │ SCS Telemetry │───┼──shm──┼─▶│ Telemetry in  │      │ long  │          │
│  │ SDK plugin    │   │       │  └───────┬───────┘      │ output│          │
│  └───────────────┘   │       │          │              │ report│          │
│                      │       │          ▼              │ (0x11)│          │
│                      │       │  ┌───────────────┐      │       │          │
│  ┌───────────────┐   │       │  │ FFB engine    │──────┼──────▶│          │
│  │ Joystick read │◀──┼─HID───┼──│ (procedural)  │      │       │          │
│  │ (axes, btns)  │   │  via  │  └───────┬───────┘      │       │          │
│  └───────────────┘   │ macOS │          │              │       │          │
│                      │       │  ┌───────▼───────┐      │       │          │
└──────────────────────┘       │  │ hidapi writer │──────┼──────▶│          │
                               │  └───────────────┘      │       │          │
                               │                         │       │          │
                               │  ┌───────────────┐      │ HID++ │          │
                               │  │ hidapi reader │◀─────┼───────│          │
                               │  │ (HID++ reply, │      │ reply │          │
                               │  │  async events)│      │       │          │
                               │  └───────────────┘      │       │          │
                               └─────────────────────────┘       └──────────┘
```

## Componentes del daemon

### 1. HID++ client (core, ya tiene la base en `src/ping.rs`)

- Abre la colección HID++ long (`usage_page=0xFF43`, `usage=0x0602`)
- Envía comandos HID++ 4.2 con framing correcto
- Correlaciona responses por `sw_id`
- Maneja el quirk del report ID `0x12` en replies del G923
- Mantiene cache de feature indexes después del discovery inicial

### 2. FFB engine (Fase 2d)

Genera efectos a partir de **inputs procedurales** (datos del juego) y los traduce a los comandos HID++ del feature `ForceFeedback 0x8123`:

- `constant_force(magnitude, direction)` — efecto constante, base del FFB de truck sims (centering proporcional a velocidad)
- `spring(strength, deadzone)` — auto-centering tipo resorte
- `damper(strength)` — resistencia rotacional proporcional a velocidad angular del volante
- `friction(strength)` — fricción estática
- `bump(intensity, duration)` — pulso corto para baches (constant force con envelope)

Los 4 slots de hardware del wheel ejecutan efectos concurrentes; el engine los asigna dinámicamente.

### 3. Telemetry ingestor (Fase 3/4)

Abstracción sobre fuentes de telemetría del juego:

- **SCS SDK (ATS/ETS2)** — plugin `.dylib` que carga el juego y publica estado en memoria compartida (speed, rpm, steering input, road surface, cargo mass, brake/clutch pressure, etc.)
- **UDP telemetry** (para Dirt Rally, F1, etc., eventualmente) — escucha puerto local, parsea estructuras documentadas
- **Shared memory APIs** (AC, ACC, AMS2) — abre mmap de los segmentos que los juegos exponen en Windows; en macOS puede no aplicar si corren vía Sikarugir

Cada ingestor normaliza a una estructura común `TelemetryFrame { speed, steering, surface, load, ... }` que el FFB engine consume.

### 4. Input reader (observacional, Fase 3)

Lee el input joystick del wheel (report ID `0x01`) en paralelo con el juego. **No sustituye** al juego como consumer — simplemente nos da visibilidad del estado actual del volante para:

- Tuning del FFB (detectar over/understeer por delta entre "donde está el volante" y "donde quiere ir")
- Logging/debug
- Potencialmente: feedback loop para efectos que dependen del estado actual del wheel (spring centering asimétrico según posición)

### 5. IPC / control plane (Fase 5)

API local sobre Unix socket o TCP loopback para:

- Clientes externos (otros juegos, GUIs) enviando efectos directamente
- Live tuning del FFB (knobs de intensidad por efecto)
- Stop/start remoto del daemon
- Healthcheck

## Integración por juego

### ATS / ETS2 (nativo macOS) — Fase 4

1. Cargar el SCS Telemetry SDK plugin (archivo `.dylib` en la carpeta del juego)
2. Plugin publica telemetría en shared memory
3. Daemon abre la shared memory al detectar que el juego está corriendo (vía process watcher o reintento periódico)
4. FFB engine calcula efectos desde la telemetría y los aplica
5. El juego sigue leyendo el joystick como siempre — **ni sabe que el daemon existe**

Ventaja: el usuario no configura nada. Instala el plugin una vez, corre el daemon al arrancar sesión, y todo funciona transparente.

### Juegos nativos Mac con `GCController` / `GCRacingWheel`

`GameControllerType = 1` en el ioreg del G923 sugiere que macOS ya lo expone como racing wheel por esa API. Estos juegos:

- Leen input vía `GCRacingWheel` (trabajo ya hecho por Apple)
- **No tienen FFB accesible** porque Apple nunca completó la API de haptics para wheels
- Si el juego expone telemetría (UDP, shared memory, SDK), nuestro daemon lo engancha igual que ATS/ETS2
- Si no expone telemetría, no hay FFB con este path. Única alternativa: aproximar FFB a partir del input del propio wheel (centering por velocidad angular), que es baja calidad pero no-cero

### Racing games en Sikarugir (D3DMetal / GPTK / Wine-based) — Fase 5

Los juegos Windows corriendo vía D3DMetal no ven macOS directamente; ven una capa de traducción. Opciones, en orden de preferencia:

1. **Telemetry UDP/shared-memory** — funciona si el juego expone telemetría UDP o ACC/AC shared memory. El daemon macOS la escucha igual que ATS/ETS2. Cero cambios en el bottle.
2. **Inyección de `dinput8.dll`** — DLL dentro del bottle que reimplementa `IDirectInputDevice8::SendForceFeedbackCommand` → serializa a nuestro daemon por socket. Es el approach de CrossWheel. Más trabajo, pero necesario si el juego no expone telemetría.
3. **Hook del engine** — específico por juego. Último recurso.

Para Fase 5 empezamos por (1) y solo consideramos (2) si hay juegos prioritarios sin telemetría.

## Decisiones de diseño explícitas

### ¿Por qué daemon y no librería?

Un solo escritor al wheel previene:

- Contención de output reports entre clientes
- Carreras con `hidapi` cuando varios processes intentan `set_report` simultáneamente
- Manejo confuso del estado del wheel (quién limpió qué efecto)

El daemon **posee** el wheel. Los clientes hablan con el daemon.

### ¿Por qué Rust?

- Concurrencia segura (tokio para la event loop, async no es obligatorio pero ayuda)
- `hidapi` crate es mantenido y cross-platform
- Portable: el mismo código corre en Linux contra `hidraw` sin cambios de arquitectura, lo que facilita validar contra `new-lg4ff` empíricamente
- Binarios pequeños, sin runtime, fáciles de empaquetar

### ¿Por qué no Swift?

Swift sería idiomático en macOS y el binding IOKit es limpio, pero:

- Peor portabilidad (no podemos validar contra Linux)
- Ecosistema menor para servers/daemons
- `hidapi` en Swift implicaría FFI manual o escribir todo en `IOKit` directo

Nada contra Swift — simplemente Rust gana en este caso específico.

### ¿Por qué no `dinput8` injection desde el día 1?

Telemetry-based FFB es:

- Más simple (cero código dentro del bottle)
- Más estable (no depende de que el Wine del día sepa cargar la DLL)
- Transversal (mismo engine alimenta ATS/ETS2, juegos nativos Mac, juegos Sikarugir con telemetría)

El approach dinput8 tiene ventajas específicas (FFB nativo del juego tal cual lo diseñaron), pero es mucho más trabajo y menos portable. Lo dejamos para Fase 5 y solo si es estrictamente necesario.

## No-goals (hoy)

- **Soportar otros wheels Logitech** (G29, G920, G923 PS). Factible técnicamente y el engine se reusaría casi entero, pero primero hacemos el G923 Xbox perfecto.
- **GUI de configuración**. CLI + archivo TOML/JSON por ahora.
- **Distribución como `.app` firmado y notariado**. Binario manual; empaquetado viene después cuando el core esté estable.
- **Compatibility con Windows o Linux hosts**. El daemon podría correr en Linux contra `new-lg4ff` con algunos ajustes, pero no es prioridad.
