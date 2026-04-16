// HID++ 2.0 "hello world" for Logitech G923 Racing Wheel (Xbox variant)
// on macOS Apple Silicon via IOHIDManager (through the hidapi crate).
//
// Sends Root.GetProtocolVersion and prints the wheel's reply.
// If this works, the entire FFB path (HID++ over the long report) is
// validated and we can build the real driver on top.
//
// HID++ 2.0 long-report wire format (20 bytes total on USB):
//   [0]  report ID          = 0x11  (HID++ long)
//   [1]  device index       = 0xFF  (direct USB target)
//   [2]  feature index      = 0x00  (IRoot, always index 0)
//   [3]  function<<4 | swid = 0x11  (function 1 = getProtocolVersion, sw id 1)
//   [4]  param0
//   [5]  param1
//   [6]  ping byte          = 0x5A  (echoed back in response)
//   [7..19] zero padding
//
// Expected response (G923 Xbox replies on report 0x12, not 0x11 — quirk):
//   [1]=0xFF  [2]=0x00  [3]=0x11  [4]=major  [5]=minor  [6]=0x5A
// Or error (HID++ error = feature 0x8F):
//   [1]=0xFF  [2]=0x8F  [3]=0x11  [4]=feature  [5]=error_code

use hidapi::{DeviceInfo, HidApi, HidDevice};
use std::error::Error;
use std::time::Duration;

const LOGITECH_VID: u16 = 0x046d;
const G923_XBOX_PID: u16 = 0xc26e;

// Logitech HID++ "long" vendor collection on interface 0 of the G923.
// Confirmed from the device's report descriptor (06 43 FF  0A 02 06  85 11 ...).
const HIDPP_USAGE_PAGE: u16 = 0xFF43;
const HIDPP_LONG_USAGE: u16 = 0x0602;

const HIDPP_REPORT_ID_LONG: u8 = 0x11;
const HIDPP_LONG_SIZE: usize = 20;
// G923 Xbox quirk: send on 0x11 (20B), replies come on 0x12 (64B).
const HIDPP_REPORT_ID_G923_REPLY: u8 = 0x12;

const HIDPP_DEV_IDX_DIRECT: u8 = 0xFF;
const HIDPP_FEATURE_IROOT: u8 = 0x00;
const HIDPP_IROOT_FN_GET_PROTOCOL_VERSION: u8 = 0x1; // function 1
const HIDPP_SW_ID: u8 = 0x1; // arbitrary, must be non-zero
const HIDPP_ERROR_FEATURE: u8 = 0x8F;

fn main() -> Result<(), Box<dyn Error>> {
    let api = HidApi::new()?;

    let device = open_hidpp_long(&api)?;

    let mut req = [0u8; HIDPP_LONG_SIZE];
    req[0] = HIDPP_REPORT_ID_LONG;
    req[1] = HIDPP_DEV_IDX_DIRECT;
    req[2] = HIDPP_FEATURE_IROOT;
    req[3] = (HIDPP_IROOT_FN_GET_PROTOCOL_VERSION << 4) | HIDPP_SW_ID;
    req[4] = 0x00;
    req[5] = 0x00;
    req[6] = 0x5A; // ping byte

    println!("→ TX  {}", hex(&req));
    let written = device.write(&req)?;
    println!("      escritos {written} bytes");

    let mut buf = [0u8; 64];
    let deadline = std::time::Instant::now() + Duration::from_millis(2000);
    let mut got_reply = false;

    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let n = device.read_timeout(&mut buf, remaining.as_millis() as i32)?;
        if n == 0 {
            break;
        }

        println!("← RX  {} ({} bytes)", hex(&buf[..n]), n);

        if is_hidpp_reply_to_get_protocol_version(&buf[..n]) {
            let major = buf[4];
            let minor = buf[5];
            let ping_echo = buf[6];
            println!();
            println!("✓ HID++ versión de protocolo {major}.{minor}");
            println!("  ping echo: 0x{ping_echo:02x}  (esperado 0x5a)");
            got_reply = true;
            break;
        } else if is_hidpp_error_reply(&buf[..n]) {
            let err = buf[5];
            println!();
            println!("✗ HID++ código de error 0x{err:02x}");
            got_reply = true;
            break;
        }
    }

    if !got_reply {
        println!();
        println!("✗ Sin respuesta HID++ en 2s.");
        println!("  Causas posibles:");
        println!("  - WindowServer tiene el dispositivo abierto en exclusivo");
        println!("  - Se abrió la colección HID incorrecta (verifica con `cargo run --bin g923-enumerate`)");
        println!("  - El volante está en un modo USB distinto al esperado");
    }

    Ok(())
}

fn open_hidpp_long(api: &HidApi) -> Result<HidDevice, Box<dyn Error>> {
    // Prefer the collection that is explicitly the HID++ long vendor collection.
    let target: Option<&DeviceInfo> = api.device_list().find(|d| {
        d.vendor_id() == LOGITECH_VID
            && d.product_id() == G923_XBOX_PID
            && d.usage_page() == HIDPP_USAGE_PAGE
            && d.usage() == HIDPP_LONG_USAGE
    });

    if let Some(info) = target {
        println!(
            "✓ Abriendo colección HID++: usage_page=0x{:04x} usage=0x{:04x}",
            info.usage_page(),
            info.usage()
        );
        return Ok(info.open_device(api)?);
    }

    println!("⚠ La colección HID++ no está expuesta como dispositivo propio.");
    println!("  Fallback: abriendo la colección joystick en la interface 0.");

    let joystick: Option<&DeviceInfo> = api.device_list().find(|d| {
        d.vendor_id() == LOGITECH_VID
            && d.product_id() == G923_XBOX_PID
            && d.usage_page() == 0x01
            && d.usage() == 0x04
    });

    if let Some(info) = joystick {
        return Ok(info.open_device(api)?);
    }

    println!("⚠ Joystick collection tampoco encontrada. Probando primer match G923.");
    Ok(api.open(LOGITECH_VID, G923_XBOX_PID)?)
}

fn is_hidpp_response_frame(buf: &[u8]) -> bool {
    // Accept either the long (0x11) or the G923-quirk (0x12) report ID,
    // then validate the HID++ envelope by shape.
    buf.len() >= 7
        && (buf[0] == HIDPP_REPORT_ID_LONG || buf[0] == HIDPP_REPORT_ID_G923_REPLY)
        && buf[1] == HIDPP_DEV_IDX_DIRECT
}

fn is_hidpp_reply_to_get_protocol_version(buf: &[u8]) -> bool {
    is_hidpp_response_frame(buf)
        && buf[2] == HIDPP_FEATURE_IROOT
        && buf[3] == ((HIDPP_IROOT_FN_GET_PROTOCOL_VERSION << 4) | HIDPP_SW_ID)
}

fn is_hidpp_error_reply(buf: &[u8]) -> bool {
    is_hidpp_response_frame(buf) && buf[2] == HIDPP_ERROR_FEATURE
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
