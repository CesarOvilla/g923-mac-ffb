# Referencias externas

Recursos que alimentan el desarrollo del proyecto. Todos verificados a fecha de la sesión inicial (2026-04-15). Si alguna URL se rompe, buscar el nombre del recurso en su plataforma — es probable que haya sido movido o renombrado.

## Protocolo HID++ — la biblia

### `berarma/new-lg4ff` (Linux kernel module)

- URL: https://github.com/berarma/new-lg4ff
- Licencia: GPL
- Qué es: módulo kernel Linux con la implementación más completa de FFB para toda la gama Logitech wheels (G25, G27, G29, G923 PS, y compatibilidad con HID++ para G920/G923 Xbox via `hid-logitech-hidpp.c`).
- Por qué importa: **la referencia canónica del protocolo**. Ingeniería reversa comunitaria de años, probada contra hardware real. Si algo funciona ahí, debería funcionar en cualquier host que hable USB HID con el wheel.
- Archivos clave:
  - `hid-lg4ff.c` — efectos FFB y mode switching para wheels HID clásicos
  - Integra con el `hid-logitech-hidpp.c` del kernel para los wheels HID++ (G920, G923 Xbox)

### Linux kernel `hid-logitech-hidpp.c`

- URL: https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c
- Qué es: driver kernel que implementa el protocolo HID++ 1.0 / 2.0 / 4.2 para todos los devices Logitech que usan ese dialecto (mice Unifying, keyboards MX, wheels G920/G923 Xbox).
- Por qué importa: la **lógica de HID++ 4.2 específica** que necesitamos para el G923 Xbox vive aquí. Incluye manejo del framing short/long, feature discovery, error codes, y el path exacto de FFB para wheels HID++.
- Código relevante: buscar `g920` y `g923` en el source para encontrar quirks específicos.

### Especificación HID++ no oficial

- No existe una spec pública oficial de Logitech
- La referencia "comunitaria" más organizada está en el wiki del proyecto `Solaar` y en los comentarios de `libratbag`

## Referencias adicionales de protocolo

### Solaar

- URL: https://github.com/pwr-Solaar/Solaar
- Licencia: GPL
- Qué es: aplicación Python completa para configurar devices Unifying (ratones, teclados) vía HID++. **No maneja wheels**, pero su implementación HID++ 2.0 en Python es la más legible y documentada que existe.
- Por qué importa: leer `lib/logitech_receiver/` da una educación completa del protocolo HID++ sin el ruido del kernel code.

### libratbag

- URL: https://github.com/libratbag/libratbag
- Licencia: MIT
- Qué es: librería C para configurar mice gaming (incluyendo Logitech via HID++)
- Por qué importa: otra implementación HID++ limpia que sirve de cross-check cuando hay dudas sobre cómo encodear un comando.

## Referencias macOS específicas

### Apple — DriverKit USB

- https://developer.apple.com/documentation/usbdriverkit
- https://developer.apple.com/documentation/driverkit/creating-a-driver-using-the-driverkit-sdk
- **Relevante aunque NO la usamos**: confirma que no necesitamos DriverKit porque el wheel ya está expuesto como HID por `AppleUserUSBHostHIDDevice`.

### Apple — IOKit / IOHIDManager

- https://developer.apple.com/documentation/iokit/iohidmanager_h
- https://developer.apple.com/library/archive/documentation/DeviceDrivers/Conceptual/HID/new_api_10_5/tn2187.html (legacy pero sigue vigente)
- Lo que hay debajo del crate `hidapi` en el backend macOS. Si alguna vez el crate se queda corto, bajar a `IOKit` directo desde Rust es factible vía FFI.

### Apple — GameController framework

- https://developer.apple.com/documentation/gamecontroller/gcracingwheel
- https://developer.apple.com/documentation/gamecontroller/racing-wheel-device-support
- La API "oficial" de Apple para racing wheels, anunciada en WWDC 2022. Input funciona parcialmente para juegos que la usen; **FFB no está expuesto públicamente**. Por eso construimos nuestro propio path.

### Apple — Force Feedback framework (legacy)

- https://developer.apple.com/documentation/forcefeedback
- Framework legacy que dependía de plug-ins por dispositivo. Logitech lo soportó en la era Intel con kexts, pero nunca para Apple Silicon. **No lo usamos** — vamos por output reports HID directos.

## Proyectos userspace relacionados

### FreeTheWheel (Feral Interactive / community forks)

- URL (fork principal): https://github.com/jackhumbert/FreeTheWheel
- URL (fork con parches Apple Silicon M1): https://codeberg.org/subaksu/FreeTheWheel
- Licencia: GPL
- Qué es: herramienta original de Feral Interactive (~2012) que hace mode switching (compat → nativo) y rango de rotación en wheels Logitech en macOS.
- Por qué importa: primer precedente de hablar con Logitech wheels desde userspace macOS. Su `WheelSupports.cpp` tiene los comandos de mode switch para toda la familia (G25, G27, G29, G920), útil como referencia histórica. **No soporta el G923 Xbox específicamente.**
- Limitación: solo hace mode switch, no genera FFB runtime.

### SDL3 (logitech hidapi driver)

- URL: https://github.com/libsdl-org/SDL
- PR relevante: https://github.com/libsdl-org/SDL/pull/11598 (mergeado marzo 2025, target SDL 3.4.0)
- Qué es: SDL3 ahora incluye un driver hidapi para wheels Logitech que soporta **G29, G27, G25, DFGT, DFP** con FFB real en macOS y FreeBSD, portado de `new-lg4ff`.
- Por qué importa: **referencia C legible** de cómo hablar el protocolo Logitech HID (no HID++) desde userspace cross-platform. El G923 Xbox no está soportado ahí porque usa HID++, pero la estructura del código es igual.
- Dónde mirar: buscar en `src/joystick/hidapi/SDL_hidapi_lg4ff.c`.

### CrossWheel

- URL: https://crosswheel.seastian.com/
- Licencia: comercial (€19.99)
- Qué es: solución comercial cerrada que implementa FFB para **G29 únicamente** dentro de bottles **CrossOver** en macOS. Inyecta una DLL en el bottle que reimplementa DirectInput FFB y habla con un daemon macOS vía `IOHIDManager`.
- Por qué importa: **prueba empírica** de que el approach userspace funciona. Si tuviéramos un G29, CrossWheel resolvería el problema inmediatamente. Para el G923 Xbox no aplica (no lo soporta) y no se integra con Sikarugir/D3DMetal, solo CrossOver puro.
- Tenerlo en mente si algún día construimos la parte de Sikarugir: la arquitectura DLL-in-bottle + daemon es la misma.

## SCS Software (ATS / ETS2)

### SCS Telemetry SDK oficial

- Repositorio: https://github.com/SCSSoftware/SCSSDKDocumentation
- Qué es: SDK oficial de SCS para plugins de telemetría. El plugin es una shared library (`.dll`, `.so`, `.dylib`) que el juego carga al inicio y que recibe callbacks con el estado del juego.
- Por qué importa: **la fuente de telemetría que alimenta el FFB engine en Fase 4**. Expone velocidad, RPM, steering input, superficie, carga del camión, posición de pedales, g-forces, etc.
- Limitación: requiere compilar el plugin para macOS Apple Silicon. El SDK oficial debería soportarlo; los forks comunitarios suelen ir detrás.

### scs-sdk-plugin (community)

- URL: https://github.com/RenCloud/scs-sdk-plugin
- Qué es: plugin comunitario popular que expone la telemetría SCS a través de shared memory estructurada (fácil de consumir desde otros processes).
- Por qué importa: más práctico que el SDK oficial si queremos leer la telemetría desde un daemon separado en vez de dentro del propio plugin.

## Foros y experiencias previas

### MacRumors — Setting up G29 with macOS Ventura

- https://forums.macrumors.com/threads/setting-up-a-logitech-g29-with-macos-ventura-not-showing-in-system-settings.2370043/
- Referencia del estado "oficial" roto de wheels Logitech en macOS actual.

### SCS Software forum — Wheels en Mac

- https://forum.scssoft.com/viewtopic.php?t=334284 (M2 Sonoma, G923 inconsistente)
- https://forum.scssoft.com/viewtopic.php?t=324053 (G920 not working)
- https://forum.scssoft.com/viewtopic.php?t=245752 (ATS G29 OS X, no FFB excepto centering)

### Apple Community threads

- https://discussions.apple.com/thread/255432054 (G923 en M2 Pro)
- https://discussions.apple.com/thread/254866667 ("What happened to the promised wheel support?")

## Datos de referencia

### Logitech VID/PID table

- https://devicehunt.com/view/type/usb/vendor/046D — listado completo de todos los devices USB Logitech, útil para confirmar PIDs y descubrir variantes
- Logitech VID: `0x046d` (1133 decimal)
- G923 Xbox PIDs:
  - `0xc26d` — modo compat Xbox (requiere mode switch)
  - `0xc26e` — modo nativo PC (el que vemos nosotros)
- G923 PS PIDs:
  - `0xc266` — modo compat
  - `0xc267` — modo nativo

### Arch Wiki — Logitech Racing Wheel

- https://wiki.archlinux.org/title/Logitech_Racing_Wheel
- Mejor explicación conceptual del compat vs native mode de toda la gama.
