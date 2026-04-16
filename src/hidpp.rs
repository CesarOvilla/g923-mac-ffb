// HID++ 4.2 client for the Logitech G923 Xbox on macOS.
//
// Built on top of `hidapi` (which uses IOHIDManager on macOS). All
// framing is handled here so higher-level modules (feature discovery,
// FFB effects, ...) only talk in terms of (feature_idx, function, params).
//
// For the wire format and G923-specific quirks (reply on report 0x12)
// see docs/hidpp-protocol.md.

use hidapi::{HidApi, HidDevice};
use std::fmt;
use std::time::{Duration, Instant};

pub const LOGITECH_VID: u16 = 0x046d;
pub const G923_XBOX_PID: u16 = 0xc26e;

pub const HIDPP_USAGE_PAGE: u16 = 0xFF43;
pub const HIDPP_LONG_USAGE: u16 = 0x0602;

pub const REPORT_ID_LONG: u8 = 0x11;
/// HID++ very-long report. 64 bytes total = 4-byte envelope + 60-byte
/// params. Required for any feature command whose params exceed 16
/// bytes (e.g. the condition effects on feature 0x8123).
///
/// On the G923 Xbox, the device *also* always replies on this report ID
/// (even to long requests) — see docs/hidpp-protocol.md.
pub const REPORT_ID_VERY_LONG: u8 = 0x12;
pub const LONG_SIZE: usize = 20;
pub const VERY_LONG_SIZE: usize = 64;
pub const LONG_PARAMS_MAX: usize = LONG_SIZE - 4;
pub const VERY_LONG_PARAMS_MAX: usize = VERY_LONG_SIZE - 4;

pub const DEV_IDX_DIRECT: u8 = 0xFF;
pub const FEATURE_IROOT: u8 = 0x00;
pub const ERROR_FEATURE: u8 = 0x8F;

pub const DEFAULT_SW_ID: u8 = 0x1;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

/// Feature IDs from the HID++ 2.0 catalog.
pub const FEATURE_ID_IROOT: u16 = 0x0000;
pub const FEATURE_ID_IFEATURESET: u16 = 0x0001;
pub const FEATURE_ID_DEVICE_FW_VERSION: u16 = 0x0003;
pub const FEATURE_ID_DEVICE_NAME: u16 = 0x0005;
pub const FEATURE_ID_FORCE_FEEDBACK: u16 = 0x8123;

#[derive(Debug)]
pub enum Error {
    Hid(hidapi::HidError),
    DeviceNotFound,
    Timeout {
        feature_idx: u8,
        function: u8,
    },
    Protocol {
        feature_idx: u8,
        function: u8,
        code: u8,
    },
    FeatureNotPresent(u16),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Hid(e) => write!(f, "HID I/O error: {e}"),
            Error::DeviceNotFound => {
                write!(f, "G923 Xbox HID++ long collection not found")
            }
            Error::Timeout {
                feature_idx,
                function,
            } => write!(
                f,
                "timeout waiting for HID++ reply (feature_idx=0x{feature_idx:02x}, function={function})",
            ),
            Error::Protocol {
                feature_idx,
                function,
                code,
            } => write!(
                f,
                "HID++ error 0x{code:02x} ({}) on feature_idx=0x{feature_idx:02x} function={function}",
                protocol_error_name(*code),
            ),
            Error::FeatureNotPresent(id) => {
                write!(f, "feature 0x{id:04x} not present in device firmware")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e)
    }
}

pub fn protocol_error_name(code: u8) -> &'static str {
    match code {
        0x00 => "NoError",
        0x01 => "Unknown",
        0x02 => "InvalidArgument",
        0x03 => "OutOfRange",
        0x04 => "HWError",
        0x05 => "LogitechInternal",
        0x06 => "InvalidFeatureIndex",
        0x07 => "InvalidFunctionID",
        0x08 => "Busy",
        0x09 => "Unsupported",
        _ => "(unknown)",
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FeatureInfo {
    pub index: u8,
    pub feature_type: u8,
    pub feature_version: u8,
}

impl FeatureInfo {
    pub fn is_obsolete(&self) -> bool {
        self.feature_type & 0x80 != 0
    }
    pub fn is_sw_hidden(&self) -> bool {
        self.feature_type & 0x40 != 0
    }
    pub fn is_engineering(&self) -> bool {
        self.feature_type & 0x20 != 0
    }
}

pub struct HidppDevice {
    dev: HidDevice,
    sw_id: u8,
    timeout: Duration,
}

impl HidppDevice {
    /// Open the G923 HID++ long collection.
    pub fn open(api: &HidApi) -> Result<Self, Error> {
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == LOGITECH_VID
                    && d.product_id() == G923_XBOX_PID
                    && d.usage_page() == HIDPP_USAGE_PAGE
                    && d.usage() == HIDPP_LONG_USAGE
            })
            .ok_or(Error::DeviceNotFound)?;
        let dev = info.open_device(api)?;
        Ok(Self {
            dev,
            sw_id: DEFAULT_SW_ID,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Send an HID++ request and block until the matching reply arrives.
    ///
    /// Auto-picks between long (20B total, up to 16B params) and very-long
    /// (64B total, up to 60B params) reports based on `params.len()`.
    /// Condition effects on feature 0x8123 need 18B of params and therefore
    /// flow over very-long; constant force fits in long.
    ///
    /// The G923 also streams unrelated async reports (joystick state, etc.)
    /// on the same endpoint. We drain-and-ignore anything that is not an
    /// HID++ frame matching our (feature_idx, function | sw_id) echo.
    ///
    /// Returns the first 16 bytes of the response payload.
    pub fn send_sync(
        &self,
        feature_idx: u8,
        function: u8,
        params: &[u8],
    ) -> Result<[u8; 16], Error> {
        debug_assert!(function <= 0x0F, "HID++ function must fit in 4 bits");
        debug_assert!(
            params.len() <= VERY_LONG_PARAMS_MAX,
            "HID++ very-long payload is 60 bytes max"
        );

        let func_swid = (function << 4) | (self.sw_id & 0x0F);

        let (report_id, total_size) = if params.len() <= LONG_PARAMS_MAX {
            (REPORT_ID_LONG, LONG_SIZE)
        } else {
            (REPORT_ID_VERY_LONG, VERY_LONG_SIZE)
        };

        let mut req = [0u8; VERY_LONG_SIZE];
        req[0] = report_id;
        req[1] = DEV_IDX_DIRECT;
        req[2] = feature_idx;
        req[3] = func_swid;
        req[4..4 + params.len()].copy_from_slice(params);

        self.dev.write(&req[..total_size])?;

        let deadline = Instant::now() + self.timeout;
        let mut buf = [0u8; 64];

        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(Error::Timeout {
                    feature_idx,
                    function,
                });
            }
            let remaining = deadline - now;
            let n = self
                .dev
                .read_timeout(&mut buf, remaining.as_millis() as i32)?;
            if n == 0 {
                return Err(Error::Timeout {
                    feature_idx,
                    function,
                });
            }
            if !is_hidpp_frame(&buf[..n]) {
                continue;
            }

            // Error frame: buf[2]=0x8F, buf[3]=feature_idx, buf[4]=func_swid, buf[5]=code
            if buf[2] == ERROR_FEATURE
                && n >= 6
                && buf[3] == feature_idx
                && buf[4] == func_swid
            {
                return Err(Error::Protocol {
                    feature_idx,
                    function,
                    code: buf[5],
                });
            }

            // Success frame: buf[2]=feature_idx, buf[3]=func_swid
            if buf[2] == feature_idx && buf[3] == func_swid && n >= LONG_SIZE {
                let mut out = [0u8; 16];
                out.copy_from_slice(&buf[4..20]);
                return Ok(out);
            }
            // else: async report or reply to a different request — keep draining
        }
    }

    // --- IRoot (feature index 0, hardcoded by spec) -------------------------

    /// IRoot.GetProtocolVersion → (major, minor, ping_echo).
    pub fn get_protocol_version(&self) -> Result<(u8, u8, u8), Error> {
        let res = self.send_sync(FEATURE_IROOT, 0x1, &[0x00, 0x00, 0x5A])?;
        Ok((res[0], res[1], res[2]))
    }

    /// IRoot.GetFeature(feature_id). Returns `FeatureNotPresent` if the
    /// firmware reports index 0 for a non-IRoot feature (the HID++ 2.0
    /// convention for "not supported").
    pub fn get_feature(&self, feature_id: u16) -> Result<FeatureInfo, Error> {
        let res = self.send_sync(
            FEATURE_IROOT,
            0x0,
            &[(feature_id >> 8) as u8, feature_id as u8],
        )?;
        let index = res[0];
        if index == 0 && feature_id != FEATURE_ID_IROOT {
            return Err(Error::FeatureNotPresent(feature_id));
        }
        Ok(FeatureInfo {
            index,
            feature_type: res[1],
            feature_version: res[2],
        })
    }
}

/// Accept either the long (0x11) or the very-long (0x12) report ID,
/// then validate the HID++ envelope by shape.
fn is_hidpp_frame(buf: &[u8]) -> bool {
    buf.len() >= 4
        && (buf[0] == REPORT_ID_LONG || buf[0] == REPORT_ID_VERY_LONG)
        && buf[1] == DEV_IDX_DIRECT
}

// --- IFeatureSet (feature_id 0x0001, discovered via IRoot) ------------------

pub struct FeatureSet<'a> {
    dev: &'a HidppDevice,
    index: u8,
}

impl<'a> FeatureSet<'a> {
    pub fn new(dev: &'a HidppDevice) -> Result<Self, Error> {
        let info = dev.get_feature(FEATURE_ID_IFEATURESET)?;
        Ok(Self {
            dev,
            index: info.index,
        })
    }

    pub fn index(&self) -> u8 {
        self.index
    }

    /// IFeatureSet.GetFeatureCount → number of features (excluding IRoot).
    /// Valid indices to pass to `get_feature_id` are `1..=count`.
    pub fn get_count(&self) -> Result<u8, Error> {
        let res = self.dev.send_sync(self.index, 0x0, &[])?;
        Ok(res[0])
    }

    /// IFeatureSet.GetFeatureID(index) → (feature_id, feature_type, feature_version).
    pub fn get_feature_id(&self, index: u8) -> Result<(u16, u8, u8), Error> {
        let res = self.dev.send_sync(self.index, 0x1, &[index])?;
        let fid = (u16::from(res[0]) << 8) | u16::from(res[1]);
        Ok((fid, res[2], res[3]))
    }
}

/// Human-readable name for a Logitech HID++ 2.0 feature ID.
/// Subset relevant for the G923; unknown IDs return `"(unknown)"`.
pub fn feature_name(id: u16) -> &'static str {
    match id {
        0x0000 => "IRoot",
        0x0001 => "IFeatureSet",
        0x0002 => "IFeatureInfo",
        0x0003 => "DeviceFwVersion",
        0x0005 => "DeviceName",
        0x0006 => "DeviceGroups",
        0x0007 => "DeviceFriendlyName",
        0x0008 => "KeepAlive",
        0x0020 => "ConfigChange",
        0x0021 => "UniqueIdentifier",
        0x00C0 => "DFUControl",
        0x00C1 => "DFUControlUnsigned",
        0x00C2 => "DFUControlSigned",
        0x00D0 => "DFU",
        0x1000 => "BatteryStatus",
        0x1004 => "UnifiedBattery",
        0x1300 => "LEDControl",
        0x1802 => "DeviceReset",
        0x1803 => "GpioAccess",
        0x1806 => "ConfigurableDeviceProperties",
        0x1814 => "ChangeHost",
        0x1815 => "HostsInfo",
        0x1981 => "BacklightBrightness",
        0x18A1 => "LedSoftwareControl",
        0x1B00 => "HotKeys",
        0x1E00 => "HiddenFeatures",
        0x1E22 => "SPI",
        0x1F20 => "TemperatureMeasurement",
        0x8010 => "GKeys",
        0x8030 => "MKeys",
        0x8040 => "BacklightV2",
        0x8060 => "ReportRate",
        0x8071 => "RGBEffects",
        0x8100 => "OnboardProfiles",
        0x8123 => "ForceFeedback",
        0x8124 => "Haptics",
        _ => "(unknown)",
    }
}
