# HID++ 4.2 en el G923 Xbox — protocolo y quirks confirmados

## Versión del protocolo

**Confirmado empíricamente: HID++ 4.2.**

El wheel respondió al `Root.GetProtocolVersion` con `major=4 minor=2`. Esto significa que hablamos el protocolo HID++ 2.0+ moderno con todas las features accesibles vía feature index discovery. Es el mismo dialect que maneja `hid-logitech-hidpp.c` del kernel Linux y la base para todos los volantes, ratones y teclados Logitech recientes.

## Formato del long report

HID++ long report = **20 bytes en el wire**, incluyendo el report ID.

```
Offset  Size  Field                      Value
------  ----  -------------------------  ----------------------------------------
  0     1     Report ID                  0x11 (on request; 0x12 on G923 reply — ver quirk abajo)
  1     1     Device Index               0xFF para direct USB target
  2     1     Feature Index              0x00 = IRoot; otros valores = features descubiertas
  3     1     (Function << 4) | SW ID    function en el nibble alto, sw_id en el bajo
  4..19 16    Parameters (function-specific payload)
```

### Device Index

- `0xFF` — direct USB, el wheel conectado por cable. **Este es nuestro valor.**
- `0x01`..`0x06` — receivers Unifying con varios periféricos (no aplica al G923)
- `0x00` — receiver mismo

### Feature Index

HID++ 2.0+ no hardcodea registros — usa **feature index discovery**. Cada feature (FFB, haptics, device info, etc.) tiene una feature ID permanente (ej. `0x8123` para ForceFeedback), pero el firmware asigna índices locales que pueden cambiar entre devices. Para usar una feature, primero mandas `IRoot.GetFeature(feature_id)` → recibes el índice local actual.

- `0x00` — IRoot (siempre en índice 0; es el punto de entrada del discovery)
- `0x8F` — **marcador de error** en responses (no es una feature real)

### Function | SW ID

Un solo byte donde:

- **Bits altos (nibble 4–7)** — function number dentro de la feature (0..15)
- **Bits bajos (nibble 0–3)** — sw_id arbitrario (1..15) elegido por el cliente

El sw_id se echoea en la response. Sirve para correlacionar requests asíncronos cuando tienes múltiples en vuelo. **Debe ser no-cero** (0 está reservado para notifications).

Nuestro ping usa `(1 << 4) | 1 = 0x11` → function 1 de IRoot = `getProtocolVersion`, sw_id = 1.

## Response frame esperado

Ante un request exitoso:

```
Offset  Size  Field
------  ----  --------------------------
  0     1     Report ID (ver quirk)
  1     1     Device Index (echo, 0xFF)
  2     1     Feature Index (echo del request)
  3     1     Function | SW ID (echo exacto)
  4..19 16    Response parameters
```

Ante un error HID++:

```
Offset  Size  Field
------  ----  --------------------------
  0     1     Report ID
  1     1     Device Index (echo)
  2     1     0x8F                       ← marcador de error
  3     1     Feature Index del request original
  4     1     Function | SW ID del request original
  5     1     Error code
  6..19 14    Padding
```

Códigos de error HID++ 2.0 comunes:

| Code | Meaning |
|---|---|
| `0x00` | NoError |
| `0x01` | Unknown |
| `0x02` | InvalidArgument |
| `0x03` | OutOfRange |
| `0x04` | HWError |
| `0x05` | LogitechInternal |
| `0x06` | InvalidFeatureIndex |
| `0x07` | InvalidFunctionID |
| `0x08` | Busy |
| `0x09` | Unsupported |

## ⚠️ Quirk crítico del G923 Xbox: reply en report `0x12`

**Enviamos** `Root.GetProtocolVersion` como long report en **report ID `0x11`** (20 bytes). El wheel **respondió en report ID `0x12`** (64 bytes en el wire).

Observación de la sesión inicial:

```
→ TX  11 ff 00 11 00 00 5a 00 00 00 00 00 00 00 00 00 00 00 00 00
← RX  12 ff 00 11 04 02 5a 00 00 00 ... (padded to 64 bytes)
         ^^ ^^ ^^ ^^ ^^ ^^
         |  |  |  |  |  +- ping echo = 0x5A
         |  |  |  |  +---- minor = 2
         |  |  |  +------- major = 4        → HID++ 4.2
         |  |  +---------- function|swid echo = 0x11
         |  +------------- feature echo = 0x00 (IRoot)
         +---------------- device index echo = 0xFF
```

El payload HID++ está correcto y ubicado exactamente donde se esperaría en un reply estándar. Solo el **report ID exterior cambia de `0x11` a `0x12`** y los bytes sobrantes del buffer de 64 bytes vienen como padding cero.

### Hipótesis del porqué

El report `0x12` está declarado en el descriptor de la interface 0 como un "vendor bulk long" de 63 bytes. El G923 Xbox probablemente reutiliza ese buffer para responder a comandos HID++ — es más grande (63 vs 19 bytes de payload), lo que le da espacio para responses extendidas cuando hagan falta. La parte baja del buffer contiene la response HID++ "normal"; el resto queda sin usar.

### Cómo lo manejamos en el código

El decoder acepta **ambos** report IDs y valida la estructura HID++ por la posición de los bytes (`device_idx` en offset 1, `feature_idx` en offset 2, etc.):

```rust
fn is_hidpp_response_frame(buf: &[u8]) -> bool {
    buf.len() >= 7
        && (buf[0] == 0x11 || buf[0] == 0x12)
        && buf[1] == 0xFF // direct USB device idx
}
```

**TODO**: capturar un dump de Wireshark/USBMon en Linux con `new-lg4ff` contra el G923 Xbox para confirmar si Linux ve el mismo quirk y cómo lo maneja internamente. Probablemente `hid-logitech-hidpp.c` ya tiene lógica para esto.

## Comandos HID++ conocidos que necesitamos implementar

### Fase 2b — Feature discovery ✅ confirmada empíricamente

Implementada en `src/hidpp.rs` (`HidppDevice::get_feature`, `FeatureSet::get_count`, `FeatureSet::get_feature_id`) y corrida vía `cargo run --bin g923-discover`.

**Hallazgos del firmware del G923 Xbox corriendo en este Mac M4:**

- IRoot → índice 0 (por spec)
- IFeatureSet → índice 1
- Firmware expone **21 features** (más IRoot)

Tabla completa tal como la reporta el wheel:

```
idx      id  type  ver  flags  name
  0  0x0000  0x00    0  -      IRoot
  1  0x0001  0x00    1  -      IFeatureSet
  2  0x0003  0x00    3  -      DeviceFwVersion
  3  0x0005  0x00    0  -      DeviceName
  4  0x1e00  0x40    0  H      HiddenFeatures
  5  0x1800  0x60    0  HE     (?)
  6  0x1eb0  0x60    0  HE     (?)
  7  0x1802  0x60    0  HE     DeviceReset
  8  0x00c1  0x00    0  -      DFUControlUnsigned
  9  0x1f1f  0x60    0  HE     (?)
 10  0x8120  0x00    1  -      (?) wheel-specific
 11  0x8123  0x00    1  -      ForceFeedback        ← objetivo
 12  0x18a1  0x60    0  HE     LedSoftwareControl
 13  0x8122  0x00    0  -      (?) wheel-specific
 14  0x8124  0x60    0  HE     Haptics
 15  0x18e8  0x60    0  HE     (?)
 16  0x92d0  0x60    0  HE     (?)
 17  0x8127  0x00    1  -      (?) wheel-specific
 18  0x807a  0x00    0  -      (?) wheel-specific
 19  0x1bc0  0x00    1  -      (?)
 20  0x80a3  0x00    0  -      (?) wheel-specific
 21  0x80d0  0x00    1  -      (?) wheel-specific
```

Flags: `O` = obsolete, `H` = SW-hidden, `E` = engineering.

**Targets confirmados:**

| Feature | ID | Índice | Version | Notas |
|---|---|---|---|---|
| IRoot | `0x0000` | 0 | — | hardcoded |
| IFeatureSet | `0x0001` | 1 | 1 | |
| DeviceFwVersion | `0x0003` | 2 | 3 | version 3 del spec |
| DeviceName | `0x0005` | 3 | 0 | |
| **ForceFeedback** | **`0x8123`** | **11** | **1** | **objetivo de Fase 2c** |
| Haptics | `0x8124` | 14 | 0 | hidden/engineering |

**Targets ausentes:** `LEDControl 0x1300` no existe — el G923 no expone LEDs programables por esa feature. Si los LEDs de RPM resultan programables más adelante, probablemente viven en uno de los `0x81xx` "wheel-specific" sin identificar (candidatos: `0x8120`, `0x8122`, `0x8127`, `0x807a`, `0x80a3`, `0x80d0`).

**TODO — identificar los wheel-specific 0x81xx / 0x80xx**: sospechosos de ser wheel info, calibración de pedales, lock/range, selector de perfiles, etc. `hid-logitech-hidpp.c` en kernel Linux y `new-lg4ff` son la referencia. Por ahora no bloquean Fase 2c porque `ForceFeedback 0x8123` basta para constant force.

### IRoot.GetFeature (forma del comando ya validada)

```
Root.GetFeature(feature_id=0x8123)
  → request: feature_idx=0x00, function=0x00, params=[0x81, 0x23]
  → response: params=[feature_idx=0x0B, feature_type=0x00, feature_version=0x01]
```

### Fase 2c — Constant force ✅ confirmada empíricamente

Feature `ForceFeedback 0x8123` @ índice 11 en el firmware G923 Xbox. Function indices verificados verbatim contra `hid-logitech-hidpp.c` del kernel:

| fn | nombre | params (request) | response |
|---|---|---|---|
| 0 | GetInfo | — | `[slot_count]` (64 en G923) |
| 1 | ResetAll | — | — |
| 2 | DownloadEffect | `[slot=0, type, ...]` (14B para Constant) | `[assigned_slot]` |
| 3 | SetEffectState | `[slot, state]` | — |
| 4 | DestroyEffect | `[slot]` | — |
| 5 | GetAperture | — | `[range_be_u16]` |
| 6 | SetAperture | `[range_be_u16]` | — |
| 7 | GetGlobalGains | — | `[gain_be_u16, boost_be_u16]` |
| 8 | SetGlobalGains | `[gain_be_u16, boost_be_u16]` | — |

Los nibbles altos `0x01, 0x11, 0x21...` que el kernel usa son `(fn << 4) | sw_id=1`, exactamente nuestra encoding.

#### Layout de DownloadEffect → Constant (14 bytes)

```
offset  size  field
------  ----  ---------------------------------
  0     1     slot (0 = let device pick)
  1     1     type | flags  (0x00 CONSTANT, OR 0x80 AUTOSTART)
  2..3  2     duration ms (BE u16, 0 = infinite)
  4..5  2     start delay ms (BE u16)
  6..7  2     force (BE i16, signed — wire convention)
  8     1     attack envelope level
  9..10 2     attack envelope length ms (BE)
  11    1     fade envelope level
  12..13 2    fade envelope length ms (BE)
```

#### Constantes de tipos de efecto (idénticas al kernel)

```
0x00 CONSTANT   0x06 SPRING
0x01 SINE       0x07 DAMPER
0x02 SQUARE     0x08 FRICTION
0x03 TRIANGLE   0x09 INERTIA
0x04 SAW_UP     0x0A RAMP
0x05 SAW_DOWN   0x80 AUTOSTART (bit OR-able)
```

### ⚠ Quirks del G923 Xbox específicos vs G920 (kernel ref)

Descubiertos durante la implementación de Fase 2c. **No están documentados en `hid-logitech-hidpp.c`** porque ese código solo se prueba contra G920.

#### Quirk 1: `SetEffectState(PLAY)` es silenciado

`hidpp_ff_playback` en el kernel envía `function=3, params=[slot, 1]` para arrancar un efecto. En el G923 Xbox, el device **responde sin error pero nunca activa el motor**. La única forma de hacer que un efecto produzca torque es `DownloadEffect` con el bit `EFFECT_AUTOSTART (0x80)` en el byte de tipo.

Tres confirmaciones empíricas:
- 4 corridas con `download → play` produjeron 0 torque
- 1 corrida con `download | AUTOSTART` (sin `play`) produjo torque inmediato
- Ninguna corrida con `play` retornó error HID++

Implementación: `ForceFeedback::upload_constant` siempre setea `EFFECT_AUTOSTART`. Las funciones `play()` y `stop()` siguen expuestas pero documentadas como no-op en este firmware.

#### Quirk 2: Signo de la fuerza invertido

| convención | + force | – force |
|---|---|---|
| Linux / G920 (`hid-logitech-hidpp.c`) | derecha | izquierda |
| **G923 Xbox observado** | **izquierda** | **derecha** |

Confirmado empíricamente: enviamos `force = +16000` esperando RIGHT y el wheel rotó a la izquierda del conductor. Con `-16000` rotó a la derecha.

Implementación: `upload_constant` hace `wire_force = force.saturating_neg()` antes de empaquetar. Los callers usan la convención natural Linux (`+ = derecha`) y la lib se encarga del flip.

#### Quirk 3: Slot 0 reservado, autocentrado de fábrica activo

`HIDPP_FF_RESERVED_SLOTS = 1` (también del kernel). El primer `DownloadEffect` recibe `slot = 1` del device — `slot 0` ya está ocupado por algo del firmware. Cuando hacemos `Destroy` de nuestro constant force, el wheel **vuelve solo al centro**, lo que sugiere que ese slot 0 reservado aloja un spring de centering implícito. No es nuestro driver — es el firmware Logitech haciendo lo suyo.

Esto es buena noticia: cuando programemos centering propio en Fase 2d, vamos a poder superponerlo o reemplazarlo.

#### Quirk 2b: el flip de signo es SOLO para direcciones absolutas, no para coeficientes

Refinamiento importante descubierto durante Fase 2d (spring). El flip de signo del Quirk 2 aplica a campos que codifican **dirección absoluta** en el espacio físico:

| campo | ¿flipado? | por qué |
|---|---|---|
| constant force `force` | **sí** | signo codifica dirección (izquierda/derecha) |
| condition `center` | **sí** | es una posición absoluta en el eje |
| condition `left_coeff` / `right_coeff` | **no** | son **rigidez** (stiffness), no dirección |
| condition `left_saturation` / `right_saturation` | n/a | magnitudes u16 sin signo |
| condition `deadband` | n/a | magnitud u15 sin signo |

**Error empírico capturado**: invertir los coeficientes del spring convierte un spring estable (jala hacia el centro) en un spring negativo/inestable (empuja lejos del centro). El wheel, cualquier pequeño desplazamiento, se dispara hasta el tope físico. Síntoma inequívoco: el aro queda pegado al tope nada más ejecutar `upload_spring`.

Regla mental: el flip compensa una inversión del eje físico del wheel. Los campos que son **direcciones en ese eje** (force, center) se flipan. Los campos que son **escalares que describen la curva de respuesta** (coefficients, saturation, deadband) no.

#### Quirk 4 (no-quirk): defaults útiles al arrancar

```
GetGlobalGains → gain = 0xFFFF (max), boost = 0x0000
GetAperture    → 900°
GetInfo        → 64 slots (raw), 63 usables
```

No hace falta `SetGlobalGains` ni `SetAperture` para FFB básico. El kernel solo lee estos valores en su init y nosotros confirmamos por qué.

### Fase 3 — Parser de input

Parse del report ID `0x01` (joystick state) según el descriptor: steering (16-bit), throttle/brake/clutch (8-bit), 23 botones, hat switch de 4 bits, vendor 3 bits. Decodificación estándar de HID joystick sin HID++ involucrado.

## Referencias del protocolo

- [`hid-logitech-hidpp.c` en el kernel Linux](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c) — la biblia del protocolo
- [`new-lg4ff` (berarma)](https://github.com/berarma/new-lg4ff) — driver FFB Logitech para Linux, GPL
- [Solaar](https://github.com/pwr-Solaar/Solaar) — implementación Python completa del HID++ que puede servir como referencia legible (aunque es para input devices, no wheels)
- [libratbag / libratman](https://github.com/libratbag/libratbag) — otra implementación HID++ en C
