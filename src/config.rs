// Configuración del daemon FFB via archivo TOML.
//
// Busca g923.toml en este orden:
//   1. Ruta pasada por argumento (--config path)
//   2. ~/.config/g923/g923.toml
//   3. ./g923.toml (directorio actual)
//   4. Valores por defecto (hardcoded)
//
// Hot-reload: el daemon revisa el mtime del archivo cada 5 segundos.
// Si cambió, recarga automáticamente sin reiniciar.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ffb: FfbConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FfbConfig {
    pub global_gain: f32,
    pub update_hz: u32,
    pub spring: SpringConfig,
    pub damper: DamperConfig,
    pub lateral: LateralConfig,
    pub vibration: VibrationConfig,
    pub surface: SurfaceConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SpringConfig {
    pub base: f32,
    pub per_kmh: f32,
    pub max: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DamperConfig {
    pub base: f32,
    pub per_kmh: f32,
    pub max: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LateralConfig {
    pub gain: f32,
    pub max: f32,
    pub smoothing: f32,
    pub deadzone: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct VibrationConfig {
    pub enabled: bool,
    pub rpm_gain: f32,
    pub idle_amplitude: f32,
    pub max_amplitude: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SurfaceConfig {
    pub enabled: bool,
    pub bump_gain: f32,
    pub bump_duration_ms: u16,
    pub bump_threshold: f32,
}

// ── Defaults ─────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self { ffb: FfbConfig::default() }
    }
}

impl Default for FfbConfig {
    fn default() -> Self {
        Self {
            global_gain: 1.0,
            update_hz: 15,
            spring: SpringConfig::default(),
            damper: DamperConfig::default(),
            lateral: LateralConfig::default(),
            vibration: VibrationConfig::default(),
            surface: SurfaceConfig::default(),
        }
    }
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self { base: 2000.0, per_kmh: 150.0, max: 18000.0, threshold: 1500.0 }
    }
}

impl Default for DamperConfig {
    fn default() -> Self {
        Self { base: 1000.0, per_kmh: 80.0, max: 10000.0, threshold: 1000.0 }
    }
}

impl Default for LateralConfig {
    fn default() -> Self {
        Self { gain: 2000.0, max: 10000.0, smoothing: 0.3, deadzone: 300.0, threshold: 500.0 }
    }
}

impl Default for VibrationConfig {
    fn default() -> Self {
        Self { enabled: false, rpm_gain: 0.5, idle_amplitude: 500.0, max_amplitude: 3000.0 }
    }
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self { enabled: false, bump_gain: 1.0, bump_duration_ms: 80, bump_threshold: 0.015 }
    }
}

// ── Carga y hot-reload ───────────────────────────────────────────

pub struct ConfigLoader {
    path: Option<PathBuf>,
    last_modified: Option<SystemTime>,
    pub config: Config,
}

impl ConfigLoader {
    pub fn new(explicit_path: Option<&str>) -> Self {
        let path = find_config(explicit_path);
        let (config, mtime) = match &path {
            Some(p) => load_from_file(p),
            None => (Config::default(), None),
        };
        Self {
            path,
            last_modified: mtime,
            config,
        }
    }

    /// Revisa si el archivo cambió. Si sí, recarga y retorna true.
    pub fn check_reload(&mut self) -> bool {
        let path = match &self.path {
            Some(p) => p,
            None => return false,
        };
        let current_mtime = fs::metadata(path).ok().and_then(|m| m.modified().ok());
        if current_mtime != self.last_modified {
            let (new_config, new_mtime) = load_from_file(path);
            self.config = new_config;
            self.last_modified = new_mtime;
            true
        } else {
            false
        }
    }

    pub fn path_display(&self) -> String {
        match &self.path {
            Some(p) => p.display().to_string(),
            None => "(defaults)".into(),
        }
    }
}

fn find_config(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let path = PathBuf::from(p);
        if path.exists() { return Some(path); }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let p = Path::new(&home).join(".config/g923/g923.toml");
        if p.exists() { return Some(p); }
    }

    let local = PathBuf::from("g923.toml");
    if local.exists() { return Some(local); }

    None
}

fn load_from_file(path: &Path) -> (Config, Option<SystemTime>) {
    let mtime = fs::metadata(path).ok().and_then(|m| m.modified().ok());
    match fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<Config>(&content) {
            Ok(config) => (config, mtime),
            Err(e) => {
                eprintln!("  ⚠ Error parseando {}: {e}", path.display());
                eprintln!("    Usando valores por defecto.");
                (Config::default(), mtime)
            }
        },
        Err(e) => {
            eprintln!("  ⚠ Error leyendo {}: {e}", path.display());
            (Config::default(), mtime)
        }
    }
}

/// Genera un g923.toml con todos los valores por defecto documentados.
pub fn generate_default_toml() -> String {
    r#"# g923-mac-ffb — configuración del daemon FFB
# Edita estos valores y el daemon los recarga automáticamente (cada 5s).

[ffb]
global_gain = 1.0          # multiplicador global de todas las fuerzas (0.0–2.0)
update_hz = 15             # tasa de actualización del daemon en Hz

[ffb.spring]
base = 2000                # fuerza de autocentrado con el camión parado
per_kmh = 150              # cuánto sube el spring por cada km/h
max = 18000                # máximo spring (alcanzado a ~107 km/h)
threshold = 1500           # solo re-enviar si cambió más que esto

[ffb.damper]
base = 1000                # amortiguamiento base (anti-oscillation)
per_kmh = 80               # cuánto sube por cada km/h
max = 10000                # máximo damper
threshold = 1000           # solo re-enviar si cambió más que esto

[ffb.lateral]
gain = 2000                # intensidad de fuerza lateral en curvas
max = 10000                # máximo (previene rebotes violentos)
smoothing = 0.3            # suavizado (0.0 = sin filtro, 0.9 = muy suave)
deadzone = 300             # fuerza mínima para activar lateral
threshold = 500            # solo re-enviar si cambió más que esto

[ffb.vibration]
enabled = false            # habilitar vibración del motor por RPM
rpm_gain = 0.5             # intensidad de la vibración
idle_amplitude = 500       # vibración en idle (motor encendido, parado)
max_amplitude = 3000       # vibración máxima a RPM alto

[ffb.surface]
enabled = false            # habilitar baches por deflexión de suspensión
bump_gain = 1.0            # intensidad de los baches
bump_duration_ms = 80      # duración de cada pulso de bache
bump_threshold = 0.015     # cambio mínimo en suspensión para disparar bache
                           # subir si sientes pulsos en carretera lisa (0.03–0.05)
                           # bajar si no sientes baches reales (0.005–0.01)
"#.to_string()
}
