// Test: ¿puede libusb mandar SET_REPORT al G923 sin romper ATS?
//
// Manda un ResetAll FFB via control transfer USB directo (endpoint 0).
// No usa IOHIDManager → no debería interferir con GameController.
//
// USB SET_REPORT:
//   bmRequestType = 0x21 (class, interface, host-to-device)
//   bRequest      = 0x09 (SET_REPORT)
//   wValue        = (report_type << 8) | report_id
//   wIndex        = interface_number
//   data          = report data (sin el report ID)

use rusb::UsbContext;
use std::io::{self, BufRead, Write};
use std::time::Duration;

const G923_VID: u16 = 0x046d;
const G923_PID: u16 = 0xc26e;

const USB_REQ_SET_REPORT: u8 = 0x09;
const USB_REQ_GET_REPORT: u8 = 0x01;
const HID_REPORT_TYPE_OUTPUT: u16 = 0x02;
const HID_REPORT_TYPE_INPUT: u16 = 0x01;

const HIDPP_REPORT_LONG: u8 = 0x11;
const HIDPP_REPORT_VLONG: u8 = 0x12;
const INTERFACE_NUM: u16 = 0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙  Test USB directo — libusb SET_REPORT al G923.");
    println!();
    println!("   Abre ATS y verifica que el volante funciona.");
    println!();

    println!("══ PASO 1: Abrir device via libusb ══");
    wait_for_enter("   [Enter para abrir] ");

    let ctx = rusb::Context::new()?;
    let handle = ctx.open_device_with_vid_pid(G923_VID, G923_PID)
        .ok_or("G923 no encontrado via libusb")?;

    println!("   ✓ Device USB abierto via libusb (sin IOHIDManager).");
    println!("   → ¿Sigue funcionando el volante en ATS? (10 segundos)");
    std::thread::sleep(Duration::from_secs(10));
    wait_for_enter("   [Enter después de verificar] ");

    println!();
    println!("══ PASO 2: Mandar SET_REPORT (ResetAll FFB) ══");
    wait_for_enter("   [Enter para mandar] ");

    // HID++ long report: ResetAll = feature_idx=0x0B, function=1, sw_id=1
    let mut data = [0u8; 19];
    data[0] = 0xFF; // device index
    data[1] = 0x0B; // feature index (FFB)
    data[2] = 0x11; // (function=1 << 4) | sw_id=1

    let w_value = (HID_REPORT_TYPE_OUTPUT << 8) | HIDPP_REPORT_LONG as u16;
    let timeout = Duration::from_secs(1);

    match handle.write_control(
        0x21, // bmRequestType: class, interface, host-to-device
        USB_REQ_SET_REPORT,
        w_value,
        INTERFACE_NUM,
        &data,
        timeout,
    ) {
        Ok(n) => println!("   ✓ SET_REPORT enviado ({n} bytes escritos)"),
        Err(e) => println!("   ✗ SET_REPORT falló: {e}"),
    }

    println!("   → ¿Sigue funcionando el volante en ATS?");
    wait_for_enter("   [Enter después de verificar] ");

    println!();
    println!("══ PASO 3: Leer respuesta via GET_REPORT ══");
    wait_for_enter("   [Enter para leer] ");

    let mut resp = [0u8; 63];
    let r_value = (HID_REPORT_TYPE_INPUT << 8) | HIDPP_REPORT_VLONG as u16;

    match handle.read_control(
        0xA1, // bmRequestType: class, interface, device-to-host
        USB_REQ_GET_REPORT,
        r_value,
        INTERFACE_NUM,
        &mut resp,
        timeout,
    ) {
        Ok(n) => {
            let end = if n < 20 { n } else { 20 };
            let hex: String = resp[..end].iter()
                .map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
            println!("   ✓ GET_REPORT recibido ({n} bytes): {hex}");
        }
        Err(e) => println!("   ✗ GET_REPORT falló: {e} (no crítico)"),
    }

    println!();
    println!("══ PASO 4: Mandar un spring FFB ══");
    wait_for_enter("   [Enter para programar spring] ");

    // Spring via very long report
    let mut spring_data = [0u8; 63];
    spring_data[0] = 0xFF; // device index
    spring_data[1] = 0x0B; // feature index
    spring_data[2] = 0x21; // function=2 (download) << 4 | sw_id=1
    // params[0] = slot 0 (let device pick)
    spring_data[3 + 1] = 0x06 | 0x80; // SPRING | AUTOSTART
    // coefficient = 15000 = 0x3A98
    spring_data[3 + 8] = 0x3A;  // left_coeff high
    spring_data[3 + 9] = 0x98;  // left_coeff low
    spring_data[3 + 14] = 0x3A; // right_coeff high
    spring_data[3 + 15] = 0x98; // right_coeff low
    // saturation = 0xFFFF
    spring_data[3 + 6] = 0x7F;  // left_sat high (>>9)
    spring_data[3 + 7] = 0xFF;  // left_sat low ((>>1)&0xFF)
    spring_data[3 + 16] = 0x7F; // right_sat high
    spring_data[3 + 17] = 0xFF; // right_sat low

    let w_value_vl = (HID_REPORT_TYPE_OUTPUT << 8) | HIDPP_REPORT_VLONG as u16;
    match handle.write_control(
        0x21,
        USB_REQ_SET_REPORT,
        w_value_vl,
        INTERFACE_NUM,
        &spring_data,
        timeout,
    ) {
        Ok(n) => println!("   ✓ Spring SET_REPORT enviado ({n} bytes)"),
        Err(e) => println!("   ✗ Spring SET_REPORT falló: {e}"),
    }

    println!("   → ¿Sientes resistencia al girar el volante en ATS?");
    println!("   → ¿Sigue funcionando el input del volante en ATS?");
    wait_for_enter("   [Enter para cerrar] ");

    // Cleanup: ResetAll
    let _ = handle.write_control(0x21, USB_REQ_SET_REPORT, w_value,
        INTERFACE_NUM, &data, timeout);

    println!("✓ Test completo.");
    Ok(())
}

fn wait_for_enter(prompt: &str) {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}
