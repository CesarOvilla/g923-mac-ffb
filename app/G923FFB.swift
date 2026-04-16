// G923 FFB — Menu bar app para macOS.
//
// Wrapper gráfico opcional para el daemon g923-daemon.
// Muestra estado en la barra de menú y permite iniciar/detener
// sin abrir Terminal.
//
// Compilar:
//   swiftc -O -o G923FFB G923FFB.swift -framework Cocoa
//
// El binario G923FFB debe estar junto a g923-daemon en el mismo directorio.

import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var daemonProcess: Process?
    var statusMenuItem: NSMenuItem!
    var telemetryMenuItem: NSMenuItem!
    var toggleMenuItem: NSMenuItem!
    var timer: Timer?

    let daemonName = "g923-daemon"

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        updateIcon(running: false)

        let menu = NSMenu()

        statusMenuItem = NSMenuItem(title: "● Detenido", action: nil, keyEquivalent: "")
        statusMenuItem.isEnabled = false
        menu.addItem(statusMenuItem)

        telemetryMenuItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        telemetryMenuItem.isEnabled = false
        telemetryMenuItem.isHidden = true
        menu.addItem(telemetryMenuItem)

        menu.addItem(NSMenuItem.separator())

        toggleMenuItem = NSMenuItem(title: "▶ Iniciar", action: #selector(toggleDaemon), keyEquivalent: "s")
        toggleMenuItem.target = self
        menu.addItem(toggleMenuItem)

        menu.addItem(NSMenuItem.separator())

        let configItem = NSMenuItem(title: "⚙ Abrir configuración…", action: #selector(openConfig), keyEquivalent: ",")
        configItem.target = self
        menu.addItem(configItem)

        let logItem = NSMenuItem(title: "📋 Ver log…", action: #selector(openLog), keyEquivalent: "l")
        logItem.target = self
        menu.addItem(logItem)

        menu.addItem(NSMenuItem.separator())

        let quitItem = NSMenuItem(title: "Salir", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem.menu = menu

        // Revisar estado cada 3 segundos
        timer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
        updateStatus()
    }

    func daemonPath() -> String {
        let bundle = Bundle.main.bundlePath
        let dir = (bundle as NSString).deletingLastPathComponent
        return (dir as NSString).appendingPathComponent(daemonName)
    }

    func configPath() -> String {
        let home = NSHomeDirectory()
        let configDir = (home as NSString).appendingPathComponent(".config/g923/g923.toml")
        if FileManager.default.fileExists(atPath: configDir) {
            return configDir
        }
        // Buscar junto al binario
        let dir = (Bundle.main.bundlePath as NSString).deletingLastPathComponent
        let local = (dir as NSString).appendingPathComponent("g923.toml")
        if FileManager.default.fileExists(atPath: local) {
            return local
        }
        return configDir // default aunque no exista
    }

    func isDaemonRunning() -> Bool {
        let pipe = Pipe()
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        process.arguments = ["-f", daemonName]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0
        } catch {
            return false
        }
    }

    func readLogTail() -> String? {
        let logPath = "/tmp/g923-ffb.log"
        guard FileManager.default.fileExists(atPath: logPath),
              let data = FileManager.default.contents(atPath: logPath),
              let content = String(data: data, encoding: .utf8) else {
            return nil
        }
        let lines = content.components(separatedBy: "\n").filter { !$0.isEmpty }
        // Buscar la última línea de status (tiene "km/h")
        return lines.last { $0.contains("km/h") }
    }

    func updateIcon(running: Bool) {
        if let button = statusItem.button {
            button.title = running ? "🟢 G923" : "⚫ G923"
        }
    }

    func updateStatus() {
        let running = isDaemonRunning()
        updateIcon(running: running)

        if running {
            statusMenuItem.title = "● Activo"
            toggleMenuItem.title = "⏹ Detener"

            if let telemetry = readLogTail() {
                telemetryMenuItem.title = telemetry.trimmingCharacters(in: .whitespaces)
                telemetryMenuItem.isHidden = false
            } else {
                telemetryMenuItem.isHidden = true
            }
        } else {
            statusMenuItem.title = "● Detenido"
            toggleMenuItem.title = "▶ Iniciar"
            telemetryMenuItem.isHidden = true
        }
    }

    @objc func toggleDaemon() {
        if isDaemonRunning() {
            stopDaemon()
        } else {
            startDaemon()
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in
            self?.updateStatus()
        }
    }

    func startDaemon() {
        let path = daemonPath()
        guard FileManager.default.fileExists(atPath: path) else {
            let alert = NSAlert()
            alert.messageText = "Daemon no encontrado"
            alert.informativeText = "No se encontró g923-daemon en:\n\(path)\n\nCompila con: cargo build --release"
            alert.runModal()
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: path)

        // Directorio de trabajo: donde está la config
        let configFile = configPath()
        let workDir = (configFile as NSString).deletingLastPathComponent
        if FileManager.default.fileExists(atPath: workDir) {
            process.currentDirectoryURL = URL(fileURLWithPath: workDir)
        }

        // Log a archivo
        let logPath = "/tmp/g923-ffb.log"
        FileManager.default.createFile(atPath: logPath, contents: nil)
        let logHandle = FileHandle(forWritingAtPath: logPath)
        process.standardOutput = logHandle
        process.standardError = logHandle

        do {
            try process.run()
            daemonProcess = process
        } catch {
            let alert = NSAlert()
            alert.messageText = "Error al iniciar"
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
    }

    func stopDaemon() {
        // Matar por nombre (cubre tanto nuestro proceso como uno lanzado por CLI)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        process.arguments = ["-f", daemonName]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try? process.run()
        process.waitUntilExit()
        daemonProcess = nil
    }

    @objc func openConfig() {
        let path = configPath()
        if FileManager.default.fileExists(atPath: path) {
            NSWorkspace.shared.open(URL(fileURLWithPath: path))
        } else {
            let alert = NSAlert()
            alert.messageText = "Config no encontrada"
            alert.informativeText = "Inicia el daemon primero para generar g923.toml"
            alert.runModal()
        }
    }

    @objc func openLog() {
        let logPath = "/tmp/g923-ffb.log"
        if FileManager.default.fileExists(atPath: logPath) {
            // Abrir en Terminal con tail -f
            let script = "tell application \"Terminal\" to do script \"tail -f \(logPath)\""
            if let appleScript = NSAppleScript(source: script) {
                var error: NSDictionary?
                appleScript.executeAndReturnError(&error)
            }
        }
    }

    @objc func quit() {
        // No matar el daemon al salir — sigue corriendo en background
        NSApp.terminate(nil)
    }
}

// ── Entry point ──────────────────────────────────────────────────

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // sin icono en el Dock
app.run()
