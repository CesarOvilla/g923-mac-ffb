// Lector de shared memory para la telemetría de ATS/ETS2.
//
// El plugin g923_telemetry.dylib (cargado dentro del proceso del juego)
// escribe un struct G923Telemetry a POSIX shared memory (/g923_telemetry)
// cada frame. Este módulo lo abre como read-only y lo expone al daemon.
//
// Layout del struct debe coincidir EXACTAMENTE con plugin/g923_telemetry.c.

use std::ptr;

pub const SHM_NAME: &[u8] = b"/g923_telemetry\0";
pub const TELEMETRY_MAGIC: u32 = 0x47393233;
pub const TELEMETRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct G923Telemetry {
    pub magic: u32,
    pub version: u32,
    pub frame: u64,
    pub speed: f32,
    pub rpm: f32,
    pub steering: f32,
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub susp_deflection: [f32; 4],
    pub on_ground: [u8; 4],
    pub cargo_mass: f32,
    pub paused: u8,
    pub _pad: [u8; 3],
}

pub struct TelemetryReader {
    ptr: *const G923Telemetry,
    size: usize,
    last_frame: u64,
}

impl TelemetryReader {
    pub fn open() -> Result<Self, String> {
        unsafe {
            let name = SHM_NAME.as_ptr() as *const i8;
            let fd = libc::shm_open(name, libc::O_RDONLY, 0);
            if fd < 0 {
                return Err(format!(
                    "shm_open({}) falló (errno {}). ¿Está ATS corriendo con el plugin?",
                    std::str::from_utf8(&SHM_NAME[..SHM_NAME.len() - 1]).unwrap_or("?"),
                    *libc::__error(),
                ));
            }

            let size = std::mem::size_of::<G923Telemetry>();
            let ptr = libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ,
                libc::MAP_SHARED,
                fd,
                0,
            );
            libc::close(fd);

            if ptr == libc::MAP_FAILED {
                return Err("mmap falló".into());
            }

            let tptr = ptr as *const G923Telemetry;
            let magic = (*tptr).magic;
            if magic != TELEMETRY_MAGIC {
                libc::munmap(ptr as *mut _, size);
                return Err(format!(
                    "magic incorrecto: 0x{magic:08x} (esperado 0x{TELEMETRY_MAGIC:08x})"
                ));
            }

            Ok(Self {
                ptr: tptr,
                size,
                last_frame: 0,
            })
        }
    }

    pub fn read(&mut self) -> G923Telemetry {
        unsafe {
            let t = ptr::read_volatile(self.ptr);
            self.last_frame = t.frame;
            t
        }
    }

    pub fn has_new_frame(&self) -> bool {
        unsafe {
            let frame_ptr = ptr::addr_of!((*self.ptr).frame);
            ptr::read_volatile(frame_ptr) != self.last_frame
        }
    }
}

impl Drop for TelemetryReader {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut _, self.size);
        }
    }
}
