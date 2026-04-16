// Fase 2b — feature discovery.
//
// Prints the full table of HID++ features exposed by the G923 Xbox
// firmware, plus a summary of "targets of interest" (ForceFeedback,
// DeviceName, DeviceFwVersion, ...). This is the reference lookup
// every subsequent phase of the driver starts from.

use g923_mac_ffb::hidpp::{
    feature_name, protocol_error_name, Error, FeatureSet, HidppDevice,
    FEATURE_ID_DEVICE_FW_VERSION, FEATURE_ID_DEVICE_NAME, FEATURE_ID_FORCE_FEEDBACK,
};
use hidapi::HidApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = HidApi::new()?;
    let dev = HidppDevice::open(&api)?;

    let (major, minor, ping) = dev.get_protocol_version()?;
    println!("✓ HID++ {major}.{minor} (ping echo 0x{ping:02x})");
    println!();

    let fset = FeatureSet::new(&dev)?;
    println!(
        "✓ IFeatureSet en feature_idx={} (0x{:02x})",
        fset.index(),
        fset.index()
    );

    let count = fset.get_count()?;
    println!("  el firmware expone {count} features (más IRoot en índice 0)");
    println!();

    print_header();
    print_row(0, 0x0000, 0x00, 0x00);

    let mut targets: Vec<TargetHit> = vec![
        TargetHit::new(FEATURE_ID_DEVICE_FW_VERSION),
        TargetHit::new(FEATURE_ID_DEVICE_NAME),
        TargetHit::new(FEATURE_ID_FORCE_FEEDBACK),
        TargetHit::new(0x8124), // Haptics
        TargetHit::new(0x1300), // LEDControl
    ];

    for i in 1..=count {
        match fset.get_feature_id(i) {
            Ok((fid, ftype, fver)) => {
                print_row(i, fid, ftype, fver);
                for t in targets.iter_mut() {
                    if t.feature_id == fid {
                        t.index = Some(i);
                    }
                }
            }
            Err(e) => {
                println!("{:>5}  (error: {})", i, format_err(&e));
            }
        }
    }

    println!();
    println!("Targets de interés:");
    for t in &targets {
        match t.index {
            Some(i) => println!(
                "  ✓ {} (0x{:04x}) @ índice {}",
                feature_name(t.feature_id),
                t.feature_id,
                i
            ),
            None => println!(
                "  ✗ {} (0x{:04x}) — NO PRESENTE",
                feature_name(t.feature_id),
                t.feature_id
            ),
        }
    }

    println!();
    println!("Leyenda de flags: O=obsoleto, H=oculto SW, E=engineering");

    Ok(())
}

struct TargetHit {
    feature_id: u16,
    index: Option<u8>,
}

impl TargetHit {
    fn new(feature_id: u16) -> Self {
        Self {
            feature_id,
            index: None,
        }
    }
}

fn print_header() {
    println!(
        "{:>5}  {:>6}  {:>4}  {:>3}  {:<5}  {}",
        "idx", "id", "tipo", "ver", "flags", "nombre"
    );
    println!(
        "{:->5}  {:->6}  {:->4}  {:->3}  {:-<5}  {:-<30}",
        "", "", "", "", "", ""
    );
}

fn print_row(idx: u8, fid: u16, ftype: u8, fver: u8) {
    let mut flags = String::new();
    if ftype & 0x80 != 0 {
        flags.push('O');
    }
    if ftype & 0x40 != 0 {
        flags.push('H');
    }
    if ftype & 0x20 != 0 {
        flags.push('E');
    }
    if flags.is_empty() {
        flags.push('-');
    }
    println!(
        "{:>5}  0x{:04x}  0x{:02x}  {:>3}  {:<5}  {}",
        idx,
        fid,
        ftype,
        fver,
        flags,
        feature_name(fid)
    );
}

fn format_err(e: &Error) -> String {
    match e {
        Error::Protocol { code, .. } => {
            format!("HID++ err 0x{:02x} {}", code, protocol_error_name(*code))
        }
        Error::Timeout { .. } => "timeout".into(),
        Error::Hid(h) => format!("hid: {h}"),
        Error::DeviceNotFound => "dispositivo no encontrado".into(),
        Error::FeatureNotPresent(id) => format!("feature 0x{id:04x} no presente"),
    }
}
