use hidapi::HidApi;
use std::error::Error;

const LOGITECH_VID: u16 = 0x046d;
const G923_XBOX_PID: u16 = 0xc26e;

fn main() -> Result<(), Box<dyn Error>> {
    let api = HidApi::new()?;

    let mut count = 0;
    for info in api.device_list() {
        if info.vendor_id() != LOGITECH_VID || info.product_id() != G923_XBOX_PID {
            continue;
        }
        count += 1;

        println!("#{count}");
        println!(
            "  vid:pid        {:04x}:{:04x}",
            info.vendor_id(),
            info.product_id()
        );
        println!(
            "  fabricante     {}",
            info.manufacturer_string().unwrap_or("(ninguno)")
        );
        println!(
            "  producto       {}",
            info.product_string().unwrap_or("(ninguno)")
        );
        println!(
            "  serie          {}",
            info.serial_number().unwrap_or("(ninguna)")
        );
        println!(
            "  usage_page     0x{:04x}  ({})",
            info.usage_page(),
            describe_usage_page(info.usage_page())
        );
        println!(
            "  usage          0x{:04x}  ({})",
            info.usage(),
            describe_usage(info.usage_page(), info.usage())
        );
        println!("  interface_no   {}", info.interface_number());
        println!("  release        0x{:04x}", info.release_number());
        println!("  path           {:?}", info.path());
        println!();
    }

    if count == 0 {
        println!("✗ G923 Xbox (046d:c26e) no encontrado.");
        println!("  Verifica que el volante esté conectado y encendido.");
    } else {
        println!("Encontradas {count} colección(es) HID del G923.");
        println!();
        println!("Para HID++ queremos: usage_page=0xff43, usage=0x0602 (HID++ long reports)");
    }

    Ok(())
}

fn describe_usage_page(page: u16) -> &'static str {
    match page {
        0x01 => "Generic Desktop",
        0x02 => "Simulation",
        0x08 => "LED",
        0x09 => "Button",
        0x0C => "Consumer",
        0xFF00..=0xFFFF => "Vendor-defined",
        _ => "other",
    }
}

fn describe_usage(page: u16, usage: u16) -> &'static str {
    match (page, usage) {
        (0x01, 0x04) => "Joystick",
        (0x01, 0x05) => "Gamepad",
        (0x01, 0x08) => "Multi-axis Controller",
        (0xFF43, 0x0602) => "Logitech HID++ long reports",
        (0xFF43, 0x0604) => "Logitech HID++ DFU / bulk",
        _ => "",
    }
}
