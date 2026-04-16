# Hardware discovery — Logitech G923 Xbox en Apple Silicon

Resultados de la sesión inicial de reconocimiento del device (2026-04-15, Mac Mini M4).

## Identidad USB

```
USB Vendor Name   Logitech
USB Product Name  G923 Racing Wheel for Xbox One and PC
idVendor          0x046d (1133)
idProduct         0xc26e (49774)
bcdDevice         0x3901  (firmware version)
Serial Number     0000e4c5a3c11afc
bDeviceClass      0x00 (composite)
bNumConfigurations 1
USBSpeed          Full Speed (12 Mbps)
```

### Dato crítico: PID `0xc26e`

El G923 Xbox tiene dos PIDs posibles:

- `0xc26d` — **modo compat Xbox**. Es el estado "fábrica" cuando arranca contra una consola Xbox. Requiere `usb_modeswitch` (mensaje vendor-specific USB control transfer) para pasar a modo nativo. Este path es el que habría requerido DriverKit en macOS.
- `0xc26e` — **modo nativo PC**. El wheel expone HID++ 4.2 + joystick HID directamente. Este es el que vemos.

En nuestro Mac M4, el wheel aparece directamente en `0xc26e`. No sabemos con certeza si:
1. macOS está haciendo el mode switch automáticamente (probable vía `GameController.framework` / `AppleUserHIDDrivers`), o
2. El firmware del G923 arranca en modo nativo cuando detecta que no está conectado a una consola Xbox

En cualquier caso, **la Fase 1 (mode switch, potencialmente requiriendo DriverKit) se salta por completo**. Este es el mejor escenario posible para empezar.

## Estructura del device compuesto

El G923 expone **dos interfaces USB** bajo el mismo device composite:

```
G923 Racing Wheel (0x046d:0xc26e)
├── IOUSBHostInterface@0  (bInterfaceNumber = 0, bInterfaceClass = 3 HID)
│   └── AppleUserUSBHostHIDDevice (claimed by com.apple.AppleUserHIDDrivers)
│       ├── IOHIDInterface — Generic Desktop / Joystick
│       └── AppleUserHIDEventDriver — GameControllerType = 1
│
└── IOUSBHostInterface@1  (bInterfaceNumber = 1, bInterfaceClass = 3 HID)
    └── AppleUserUSBHostHIDDevice
        └── IOHIDInterface — Vendor-defined (DFU / bulk)
```

Puntos clave:

- **Ambas interfaces son HID estándar** (bInterfaceClass = 3). No hay bulk USB pelado — todo pasa por el stack HID del kernel.
- **`AppleUserUSBHostHIDDevice`** es la implementación moderna DriverKit de Apple del driver HID. Reemplazó el antiguo `IOUSBHIDDriver` kext.
- `UsbExclusiveOwner = "AppleUserUSBHostHIDDevice"` en ambas interfaces → Apple tiene ownership exclusivo del USB interface. No podemos bajar a `IOUSBLib` para control transfers crudos, pero **no hace falta** — trabajamos en la capa HID.
- `DeviceOpenedByEventSystem = Yes` → WindowServer mantiene el device abierto en modo compartido. `IOHIDDeviceOpen` con `kIOHIDOptionsTypeNone` coexiste; solo `kIOHIDOptionsTypeSeizeDevice` lo rompería.
- **`GameControllerType = 1`** → `GameController.framework` clasifica este wheel como game controller y probablemente lo expone como `GCRacingWheel`. Para juegos nativos Mac que usen esa API, input (axes/buttons) llega gratis sin tocar nada.

## Las 4 colecciones HID enumeradas

`cargo run --bin g923-enumerate` retorna **4 entradas** para `046d:c26e` porque el device tiene dos interfaces y la interface 0 declara múltiples top-level collections en su report descriptor. `IOHIDManager` / `hidapi` enumera cada collection por separado.

| # | Interface | Usage Page | Usage | Significado |
|---|---|---|---|---|
| 1 | 1 | `0xFFFD` | `0xFD01` | Vendor-defined bulk channel (probablemente DFU del firmware) |
| 2 | 0 | `0x0001` Generic Desktop | `0x0004` Joystick | **Axes, pedales, botones, hat switch** — lo que los juegos leen como "wheel" |
| 3 | 0 | `0xFF43` | `0x0602` | **HID++ long reports** — canal de control del wheel (FFB, LEDs, config) ← **NUESTRO TARGET** |
| 4 | 0 | `0xFF43` | `0x0604` | HID++ DFU / bulk en el mismo interface |

### Cómo lo usamos

- Para **FFB, feature discovery, config**: abrir colección #3 (`0xFF43 / 0x0602`) y escribir output reports con report ID `0x11`. Es lo que hace `src/ping.rs`.
- Para **leer input del wheel** (steering angle, throttle, brake, clutch, buttons, hat): abrir colección #2 (`0x01 / 0x04`) y leer input reports con report ID `0x01`. Aún no implementado — siguiente fase.
- Colecciones #1 y #4 son DFU/bulk del firmware. No las tocamos.

## Report descriptor — interface 0 decodificado

Extracto relevante del descriptor de la interface 0 (bytes crudos en el ioreg; decodificación manual):

```
05 01            Usage Page (Generic Desktop)
09 04            Usage (Joystick)
A1 01            Collection (Application)
  A1 02            Collection (Logical)
    85 01              Report ID 0x01
    ... (hat switch 4-bit, 23 botones, 2-byte steering X,
         1-byte throttle/brake/clutch Y/Z/Rz, 3-bit vendor)
  C0               End Collection
C0               End Collection

06 43 FF         Usage Page 0xFF43 (Logitech vendor)
0A 02 06         Usage 0x0602
A1 01            Collection (Application)
  85 11              Report ID 0x11 (HID++ long)
  75 08              Report Size 8 bits
  95 13              Report Count 0x13 = 19
  15 00 26 FF 00     Logical Min 0 Max 255
  09 02              Usage 0x02
  81 00              Input (19 bytes)
  09 02              Usage 0x02
  91 00              Output (19 bytes)
C0               End Collection

06 43 FF         Usage Page 0xFF43
0A 04 06         Usage 0x0604
A1 01            Collection (Application)
  85 12              Report ID 0x12
  75 08              Report Size 8
  95 3F              Report Count 63
  ... (63-byte Input + 63-byte Output)
C0               End Collection
```

Equivale a:

- **Report ID `0x01`**: input-only, joystick state, ~11 bytes después del ID
- **Report ID `0x11`**: input + output, **19 bytes de payload → 20 bytes en el wire con el report ID**. Este es el canal HID++ long.
- **Report ID `0x12`**: input + output, **63 bytes de payload → 64 bytes en el wire**. Documentado como DFU/bulk, PERO el G923 Xbox lo usa también para **responder a requests HID++ que enviamos por 0x11** (ver quirk en `hidpp-protocol.md`).

## `InputReportElements` según macOS

`ioreg` reporta los input reports que macOS decodifica del descriptor:

```
Report ID 1  — 88 bits (11 bytes)  → joystick state (axes + buttons)
Report ID 17 — 160 bits (20 bytes) → HID++ long  (0x11)
Report ID 18 — 512 bits (64 bytes) → vendor long (0x12)
```

Los 160 bits de report 17 incluyen cabecera del propio report. El payload útil de HID++ son los 19 bytes después del report ID.

## Estado del device al arrancar el daemon

- `"HIDRMDeviceState" = "Approving"` — macOS aprueba el device como trusted
- `"DeviceOpenedByEventSystem" = "Yes"` — input está fluyendo al sistema de eventos
- `"RegisterService" = "No"` — atributo interno DriverKit, irrelevante para nosotros
- Power management estable, MaxPowerState = 3

## Comandos de diagnóstico útiles

```bash
# Lista completa de device USB con VID 046d
system_profiler SPUSBDataType | grep -A 30 "046d\|logitech\|G923"

# Árbol IOKit del G923 con todas las propiedades
ioreg -l -w0 | grep -B 5 -A 60 "G923 Racing Wheel"

# Solo devices HID registrados
ioreg -c IOHIDDevice -l -w0 | grep -B 5 -A 40 "G923\|046d"

# Enumeración del proyecto (rápido, legible)
cargo run --bin g923-enumerate
```
