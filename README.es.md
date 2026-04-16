🇺🇸 [Read in English](README.md)

# g923-mac-ffb

**Force Feedback para Logitech G923 Racing Wheel (Xbox) en macOS Apple Silicon.**

Driver userspace 100% — sin DriverKit, sin kexts, sin cuenta de desarrollador pagada. Funciona con American Truck Simulator y Euro Truck Simulator 2.

## El problema

El Logitech G923 (variante Xbox, PID `0xc26e`) no tiene soporte oficial de Force Feedback en macOS. Logitech nunca actualizó sus drivers para Apple Silicon, y macOS eliminó el soporte de kexts necesario para el framework `ForceFeedback.framework` legacy. El volante funciona como joystick (ejes, pedales, botones), pero los motores FFB quedan completamente muertos.

## La solución

Este proyecto habla directamente con el G923 vía **HID++ 4.2** (el protocolo propietario de Logitech) desde userspace, usando `hidapi` sobre `IOHIDManager`. Un plugin de telemetría corre dentro del juego, publica datos a shared memory, y un daemon externo traduce esa telemetría a comandos FFB que envía al volante.

```
┌──────────────┐   telemetría   ┌─────────────────┐   HID++ 4.2   ┌───────┐
│ ATS / ETS2   │──────────────▶ │ g923-daemon     │──────────────▶ │ G923  │
│ (plugin .dylib) shm POSIX    │ (Rust, arm64)   │  USB reports   │ Xbox  │
└──────────────┘               └─────────────────┘               └───────┘
```

## Efectos FFB soportados

| Efecto | Descripción | Uso |
|--------|-------------|-----|
| **Spring** | Autocentrado proporcional a velocidad | Más rápido = volante más firme |
| **Damper** | Resistencia a velocidad angular | Anti-oscillación, suaviza movimientos |
| **Constant Force** | Fuerza lateral en curvas | Sientes las G en las curvas |
| **Periodic Sine** | Vibración del motor por RPM | Sientes el motor vibrando |
| **Bumps** | Pulsos por deflexión de suspensión | Baches y cambios de superficie |
| **Weight** | Multiplicador por carga | Camión cargado = dirección más pesada |
| Friction | Drag constante | Disponible en la librería |
| Inertia | Masa virtual | Disponible en la librería |

## Requisitos

- **Mac con Apple Silicon** (M1, M2, M3, M4)
- **macOS Sonoma 14+** (probado en macOS 26.4/Tahoe)
- **Logitech G923 Racing Wheel** variante **Xbox/PC** (PID `0xc26e`)
- **Rust** (para compilar)
- **American Truck Simulator** o **Euro Truck Simulator 2** via Steam
- **clang** (incluido con Xcode Command Line Tools) para compilar el plugin C

> **Nota**: este proyecto es para la variante **Xbox** del G923 (`0xc26e`), no la PlayStation (`0xc266`). La variante PS usa un protocolo diferente — para esa, usa [fffb](https://github.com/eddieavd/fffb).

## Instalación rápida (desde DMG)

Descarga el `.dmg` más reciente de [Releases](../../releases), ábrelo y doble-click en **"Instalar.command"**. Copia binarios, config, plugin y servicio automáticamente. Después solo abre ATS y maneja.

## Compilar desde código fuente

### 1. Compilar

```bash
git clone https://github.com/tu-usuario/g923-mac-ffb.git
cd g923-mac-ffb
cargo build --release
```

### 2. Instalar el plugin de telemetría en ATS

El plugin es un `.dylib` x86_64 que se carga dentro del proceso del juego:

```bash
# Compilar el plugin
bash plugin/build.sh

# Copiar al directorio de plugins de ATS (requiere acceso al .app bundle)
# Si macOS bloquea la copia, hazlo manualmente desde Finder:
# Click derecho en ATS.app → Mostrar contenido → Contents/MacOS → crear carpeta "plugins"
cp plugin/g923_telemetry.dylib \
  ~/Library/Application\ Support/Steam/steamapps/common/\
  American\ Truck\ Simulator/American\ Truck\ Simulator.app/\
  Contents/MacOS/plugins/
```

> La primera vez que abras ATS con el plugin, aparece un diálogo de advertencia del SDK. Es normal — acepta y continúa.

### 3. Configurar el daemon

```bash
# Primera ejecución: genera g923.toml con valores por defecto
./target/release/g923-daemon
# (Ctrl+C después de verificar que genera el archivo)

# Opcionalmente, mover la config a ~/.config/g923/
mkdir -p ~/.config/g923
mv g923.toml ~/.config/g923/
```

### 4. Instalar como servicio (auto-start)

```bash
# Instala servicio launchctl — el daemon arranca al iniciar sesión
./target/release/g923 install-service

# Arrancarlo ahora
./target/release/g923 start

# Verificar
./target/release/g923 status
```

## Uso

### Con servicio instalado (recomendado)

1. Enciende la Mac (el daemon arranca solo)
2. Abre ATS/ETS2 desde Steam
3. Maneja — el FFB se activa automáticamente al detectar telemetría

### Manual

```bash
# Terminal 1: daemon
./target/release/g923-daemon

# Terminal 2 (o Steam): abrir ATS
# El daemon detecta la telemetría y activa FFB
```

### CLI

```bash
g923 start              # Arranca el daemon en background
g923 stop               # Detiene el daemon
g923 status             # Muestra estado
g923 log                # Muestra el log en tiempo real
g923 install-service    # Auto-start al login
g923 uninstall-service  # Quita auto-start
g923 uninstall          # Desinstala TODO (binarios, config, servicio)
g923 version            # Muestra la versión instalada
```

### App de barra de menú (opcional)

Una utilidad ligera que vive en la barra de menú. Muestra un icono verde/rojo indicando el estado del daemon, y permite iniciar/detener, abrir la config o ver logs — todo sin abrir Terminal.

```bash
~/.local/bin/G923FFB &
```

## Configuración

Edita `g923.toml` (en `~/.config/g923/` o el directorio actual). **El daemon recarga cambios automáticamente cada 5 segundos** — no necesitas reiniciar.

```toml
[ffb]
global_gain = 1.0          # multiplicador global (0.0–2.0)
update_hz = 15             # tasa de actualización

[ffb.spring]
base = 2000                # autocentrado con el camión parado
per_kmh = 150              # cuánto sube por km/h
max = 18000                # máximo

[ffb.damper]
base = 1000                # amortiguamiento base
per_kmh = 80               # sube por km/h
max = 10000                # máximo

[ffb.lateral]
gain = 2000                # intensidad en curvas
max = 10000                # máximo
smoothing = 0.3            # suavizado (0.0–0.9)

[ffb.vibration]
enabled = true             # vibración del motor por RPM
rpm_gain = 0.5             # intensidad
idle_amplitude = 500       # vibración en idle
max_amplitude = 3000       # vibración a RPM alto

[ffb.surface]
enabled = true             # baches por suspensión
bump_gain = 1.0            # intensidad
bump_threshold = 0.015     # sensibilidad (subir si hay falsos positivos)

[ffb.weight]
enabled = true             # más peso = dirección más dura
reference_mass = 20000     # kg de referencia
max_multiplier = 1.8       # multiplicador máximo
```

## Herramientas de diagnóstico

```bash
# Enumerar colecciones HID del G923
cargo run --bin g923-enumerate

# Ping HID++ (verificar comunicación)
cargo run --bin g923-ping

# Descubrir features del firmware
cargo run --bin g923-discover

# Test de efectos individuales
cargo run --bin g923-constant-force
cargo run --bin g923-spring
cargo run --bin g923-damper
cargo run --bin g923-friction
cargo run --bin g923-inertia
cargo run --bin g923-envelope

# Visor de input en tiempo real (steering, pedales, botones)
cargo run --bin g923-input

# Monitor de telemetría ATS
cargo run --bin g923-telemetry-monitor
```

## Arquitectura

```
g923-mac-ffb/
├── src/
│   ├── lib.rs              # Crate principal
│   ├── hidpp.rs            # Transporte HID++ 4.2 (long + very-long reports)
│   ├── ffb.rs              # Cliente ForceFeedback feature 0x8123
│   ├── telemetry.rs        # Lector de shared memory POSIX
│   ├── config.rs           # Parser TOML con hot-reload
│   ├── daemon.rs           # Loop principal: telemetría → FFB
│   ├── cli.rs              # CLI g923 (start/stop/status/install)
│   ├── input.rs            # Visor de input del joystick
│   └── *.rs                # Binarios de test/diagnóstico
├── plugin/
│   ├── g923_telemetry.c    # Plugin SCS (x86_64, corre dentro de ATS)
│   ├── build.sh            # Compila el plugin con clang
│   └── install.sh          # Instala el plugin en ATS
├── docs/
│   ├── hardware-discovery.md   # Dump USB, colecciones HID, descriptors
│   ├── hidpp-protocol.md       # Protocolo HID++ 4.2, quirks del G923 Xbox
│   ├── architecture.md         # Diseño del daemon
│   ├── roadmap.md              # Fases completadas y futuras
│   └── references.md           # Repos y docs de referencia
├── g923.toml               # Configuración FFB
└── Cargo.toml
```

## Quirks del G923 Xbox en macOS

Estos comportamientos son específicos de la variante Xbox (`0xc26e`) y no están documentados en ningún otro proyecto:

1. **`SetEffectState(PLAY)` es ignorado** — el firmware acepta el comando sin error pero nunca activa el efecto. La única forma de ejecutar efectos es con el bit `EFFECT_AUTOSTART (0x80)` en `DownloadEffect`.

2. **Signo de fuerza invertido** — el kernel Linux (G920) usa `+force = derecha`. El G923 Xbox usa `+force = izquierda`. La librería compensa internamente.

3. **Replies en report ID `0x12`** — el G923 responde en very-long reports (64 bytes) aunque el request vaya en long (20 bytes).

4. **`hidapi` requiere `macos-shared-device`** — sin esta feature flag, `hidapi` abre el device en modo exclusivo, desconectando el `GameController.framework` y matando el input del juego. Este fue el descubrimiento más crítico del proyecto.

5. **Colección vendor `0xFF43` invisible bajo Rosetta** — procesos x86_64 (como ATS) no ven las colecciones HID++ del G923. Por eso el FFB debe correr desde un daemon arm64 nativo, no desde un plugin in-process.

6. **`SetAperture` (lock rotation range) es ignorado** — el firmware acepta pero no cambia el rango físico. El control de rango probablemente vive en una feature `0x80xx` sin documentar.

## Limitaciones conocidas

- Solo funciona con la variante **Xbox** del G923 (`0xc26e`)
- Solo probado con **ATS** (ETS2 debería funcionar igual — mismo SDK)
- El plugin de telemetría requiere acceso al `.app` bundle de ATS (puede requerir quitar protecciones)
- No soporta protocolo clásico lg4ff (G29/G920/G923 PS usan protocolo diferente)
- Lock rotation range no funciona (usa Logitech GHub para cambiarlo)

## Referencias

- [hid-logitech-hidpp.c](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c) — driver HID++ del kernel Linux (referencia principal del protocolo)
- [new-lg4ff](https://github.com/berarma/new-lg4ff) — driver FFB clásico para Linux (NO compatible con G923 Xbox)
- [fffb](https://github.com/eddieavd/fffb) — FFB para G29/G923 PS en macOS (protocolo clásico, inspiración arquitectónica)
- [SDL3 PR #11598](https://github.com/libsdl-org/SDL/pull/11598) — FFB clásico en SDL3

## Licencia

MIT
