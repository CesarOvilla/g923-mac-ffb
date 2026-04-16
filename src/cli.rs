// CLI g923 — gestiona el daemon FFB.
//
// Subcomandos:
//   g923 start            arranca el daemon en background
//   g923 stop             detiene el daemon
//   g923 status           muestra si está corriendo
//   g923 install-service  instala launchctl plist (auto-start al login)
//   g923 uninstall-service quita el plist
//   g923 log              muestra el log del daemon
//   g923 test             corre la suite de pruebas FFB

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SERVICE_LABEL: &str = "com.g923.ffb";
const LOG_PATH: &str = "/tmp/g923-ffb.log";

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match cmd {
        "start" => cmd_start(),
        "stop" => cmd_stop(),
        "status" => cmd_status(),
        "install-service" => cmd_install_service(),
        "uninstall-service" => cmd_uninstall_service(),
        "log" => cmd_log(),
        "test" => cmd_test(),
        "uninstall" => cmd_uninstall(),
        "version" => cmd_version(),
        "help" | "--help" | "-h" => cmd_help(),
        other => {
            eprintln!("Comando desconocido: {other}");
            cmd_help();
        }
    }
}

fn cmd_help() {
    println!("g923 — gestor del daemon FFB para G923 Xbox en macOS");
    println!();
    println!("Uso: g923 <comando>");
    println!();
    println!("Comandos:");
    println!("  start              Arranca el daemon en background");
    println!("  stop               Detiene el daemon");
    println!("  status             Muestra si el daemon está corriendo");
    println!("  install-service    Instala servicio launchctl (auto-start al login)");
    println!("  uninstall-service  Quita el servicio launchctl");
    println!("  uninstall          Desinstala TODO (binarios, config, servicio)");
    println!("  version            Muestra la versión instalada");
    println!("  log                Muestra el log del daemon");
    println!("  test               Corre la suite de pruebas FFB");
    println!("  help               Muestra esta ayuda");
}

fn daemon_path() -> PathBuf {
    // Buscar g923-daemon junto al binario g923
    let self_path = env::current_exe().unwrap_or_default();
    let dir = self_path.parent().unwrap_or(Path::new("."));
    dir.join("g923-daemon")
}

fn config_dir() -> PathBuf {
    // Directorio de trabajo: donde está g923.toml
    let home = env::var("HOME").unwrap_or_default();
    let config = PathBuf::from(&home).join(".config/g923");
    if config.join("g923.toml").exists() {
        return config;
    }
    // Fallback: directorio actual
    env::current_dir().unwrap_or_default()
}

fn plist_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{SERVICE_LABEL}.plist"))
}

fn is_running() -> bool {
    let output = Command::new("launchctl")
        .args(["list", SERVICE_LABEL])
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

fn get_pid() -> Option<String> {
    let output = Command::new("launchctl")
        .args(["list", SERVICE_LABEL])
        .output()
        .ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Primera línea: "PID\tStatus\tLabel" o similar
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 && parts[2] == SERVICE_LABEL {
            let pid = parts[0].trim();
            if pid != "-" && !pid.is_empty() {
                return Some(pid.to_string());
            }
        }
    }
    None
}

fn cmd_start() {
    let daemon = daemon_path();
    if !daemon.exists() {
        eprintln!("✗ No se encuentra el daemon en: {}", daemon.display());
        eprintln!("  Compila con: cargo build --release --bin g923-daemon");
        return;
    }

    if is_running() {
        println!("⚠ El daemon ya está corriendo.");
        cmd_status();
        return;
    }

    // Si hay plist instalado, usar launchctl
    if plist_path().exists() {
        let status = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(plist_path())
            .status();
        match status {
            Ok(s) if s.success() => println!("✓ Daemon arrancado via launchctl."),
            _ => eprintln!("✗ Error al arrancar via launchctl."),
        }
    } else {
        // Sin plist: arrancar directo en background
        let work_dir = config_dir();
        let log_file = fs::File::create(LOG_PATH).unwrap_or_else(|_| {
            fs::File::create("/dev/null").unwrap()
        });
        let log_err = log_file.try_clone().unwrap_or_else(|_| {
            fs::File::create("/dev/null").unwrap()
        });

        match Command::new(&daemon)
            .current_dir(&work_dir)
            .stdout(log_file)
            .stderr(log_err)
            .spawn()
        {
            Ok(child) => {
                println!("✓ Daemon arrancado (PID {}).", child.id());
                println!("  Log: {LOG_PATH}");
                println!("  Para auto-start al login: g923 install-service");
            }
            Err(e) => eprintln!("✗ Error al arrancar: {e}"),
        }
    }
}

fn cmd_stop() {
    if plist_path().exists() && is_running() {
        let status = Command::new("launchctl")
            .args(["unload"])
            .arg(plist_path())
            .status();
        match status {
            Ok(s) if s.success() => println!("✓ Daemon detenido."),
            _ => eprintln!("✗ Error al detener via launchctl."),
        }
    } else {
        // Intentar matar por nombre
        let _ = Command::new("pkill").args(["-f", "g923-daemon"]).status();
        println!("✓ Daemon detenido (pkill).");
    }
}

fn cmd_status() {
    if is_running() {
        let pid = get_pid().unwrap_or_else(|| "?".into());
        println!("✓ Daemon corriendo (PID {pid})");
        if plist_path().exists() {
            println!("  Servicio: instalado (auto-start al login)");
        }
    } else {
        println!("✗ Daemon no está corriendo.");
        if plist_path().exists() {
            println!("  Servicio: instalado pero detenido. Usa: g923 start");
        } else {
            println!("  Servicio: no instalado. Usa: g923 install-service");
        }
    }
    println!("  Log: {LOG_PATH}");
    println!("  Config: {}", config_dir().join("g923.toml").display());
}

fn cmd_install_service() {
    let daemon = daemon_path();
    if !daemon.exists() {
        eprintln!("✗ No se encuentra el daemon en: {}", daemon.display());
        eprintln!("  Compila con: cargo build --release --bin g923-daemon");
        return;
    }

    let work_dir = config_dir();
    let plist = plist_path();

    // Crear directorio LaunchAgents si no existe
    if let Some(parent) = plist.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{SERVICE_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{daemon}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>WorkingDirectory</key>
    <string>{work_dir}</string>
    <key>StandardOutPath</key>
    <string>{LOG_PATH}</string>
    <key>StandardErrorPath</key>
    <string>{LOG_PATH}</string>
</dict>
</plist>
"#,
        daemon = daemon.display(),
        work_dir = work_dir.display(),
    );

    match fs::write(&plist, &content) {
        Ok(_) => {
            println!("✓ Servicio instalado: {}", plist.display());
            println!("  El daemon arrancará automáticamente al iniciar sesión.");
            println!("  Para arrancarlo ahora: g923 start");
        }
        Err(e) => eprintln!("✗ Error escribiendo plist: {e}"),
    }
}

fn cmd_uninstall_service() {
    // Detener si está corriendo
    if is_running() {
        cmd_stop();
    }

    let plist = plist_path();
    if plist.exists() {
        match fs::remove_file(&plist) {
            Ok(_) => println!("✓ Servicio desinstalado."),
            Err(e) => eprintln!("✗ Error eliminando plist: {e}"),
        }
    } else {
        println!("⚠ No hay servicio instalado.");
    }
}

fn cmd_uninstall() {
    println!("⚠ Esto va a desinstalar G923 FFB completamente:");
    println!("  - Detener el daemon");
    println!("  - Quitar servicio launchctl");
    println!("  - Eliminar binarios de ~/.local/bin/");
    println!("  - Eliminar config de ~/.config/g923/");
    println!("  - Eliminar log");
    println!();
    print!("¿Continuar? (s/N): ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if !input.trim().eq_ignore_ascii_case("s") {
        println!("Cancelado.");
        return;
    }

    // Detener daemon
    cmd_stop();

    // Quitar servicio
    let plist = plist_path();
    if plist.exists() {
        let _ = fs::remove_file(&plist);
        println!("  ✓ Servicio eliminado.");
    }

    // Matar la menu bar app si está corriendo
    let _ = Command::new("pkill").args(["-f", "G923FFB"]).status();

    // Eliminar binarios
    let bin_dir = env::var("HOME").unwrap_or_default();
    let bin_path = Path::new(&bin_dir).join(".local/bin");
    for name in &["g923-daemon", "g923", "G923FFB"] {
        let p = bin_path.join(name);
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
    }
    println!("  ✓ Binarios eliminados.");

    // Eliminar config
    let config = Path::new(&bin_dir).join(".config/g923");
    if config.exists() {
        let _ = fs::remove_dir_all(&config);
        println!("  ✓ Configuración eliminada.");
    }

    // Eliminar log
    let _ = fs::remove_file(LOG_PATH);

    println!();
    println!("✓ G923 FFB desinstalado completamente.");
    println!("  El plugin de ATS (g923_telemetry.dylib) sigue en el bundle del juego.");
    println!("  Puedes quitarlo manualmente si quieres.");
}

fn cmd_version() {
    println!("g923-mac-ffb v{}", env!("CARGO_PKG_VERSION"));
}

fn cmd_log() {
    if Path::new(LOG_PATH).exists() {
        let _ = Command::new("tail")
            .args(["-f", "-n", "50", LOG_PATH])
            .status();
    } else {
        println!("⚠ No hay log en {LOG_PATH}");
        println!("  Arranca el daemon primero: g923 start");
    }
}

fn cmd_test() {
    let test_bin = daemon_path().with_file_name("g923-daemon-test");
    if test_bin.exists() {
        let _ = Command::new(&test_bin).status();
    } else {
        eprintln!("✗ No se encuentra: {}", test_bin.display());
        eprintln!("  Compila con: cargo build --release --bin g923-daemon-test");
    }
}
