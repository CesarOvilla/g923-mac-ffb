#!/bin/bash
# Instalador G923 FFB para macOS
# Doble-click en este archivo para instalar todo automáticamente.

set -e
clear

echo "╔══════════════════════════════════════════════════╗"
echo "║       G923 FFB — Instalador para macOS          ║"
echo "║  Force Feedback para Logitech G923 Xbox         ║"
echo "╚══════════════════════════════════════════════════╝"
echo

DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/g923"
ATS_PLUGINS="$HOME/Library/Application Support/Steam/steamapps/common/American Truck Simulator/American Truck Simulator.app/Contents/MacOS/plugins"

# Detectar instalación previa
if [ -f "$INSTALL_DIR/g923" ]; then
    OLD_VER=$("$INSTALL_DIR/g923" version 2>/dev/null || echo "desconocida")
    echo "⬆ Actualización detectada (versión anterior: $OLD_VER)"
    echo "  Se actualizan binarios y plugin. Tu configuración NO se toca."
    echo

    # Detener daemon si está corriendo
    "$INSTALL_DIR/g923" stop 2>/dev/null || true
    sleep 1
else
    echo "📦 Instalación nueva."
    echo
fi

# 1. Crear directorios
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"

# 2. Copiar binarios
echo "→ Instalando binarios en $INSTALL_DIR..."
cp "$DIR/g923-daemon" "$INSTALL_DIR/"
cp "$DIR/g923" "$INSTALL_DIR/"
cp "$DIR/G923FFB" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/g923-daemon" "$INSTALL_DIR/g923" "$INSTALL_DIR/G923FFB"

NEW_VER=$("$INSTALL_DIR/g923" version 2>/dev/null || echo "")
echo "  Versión instalada: $NEW_VER"

# 3. Copiar config si no existe (nunca sobrescribir config del usuario)
if [ ! -f "$CONFIG_DIR/g923.toml" ]; then
    echo "→ Creando configuración por defecto..."
    cp "$DIR/g923.toml" "$CONFIG_DIR/"
else
    echo "→ Configuración existente conservada: $CONFIG_DIR/g923.toml"
fi

# 4. Instalar plugin de telemetría en ATS
echo
if [ -d "$ATS_PLUGINS" ]; then
    echo "→ Instalando plugin de telemetría en ATS..."
    cp "$DIR/g923_telemetry.dylib" "$ATS_PLUGINS/"
    echo "  ✓ Plugin instalado."
else
    echo "⚠ ATS no encontrado o la carpeta plugins no existe."
    echo "  Copia manualmente g923_telemetry.dylib a:"
    echo "  ATS.app/Contents/MacOS/plugins/"
    echo
    echo "  Para crear la carpeta plugins:"
    echo "  Click derecho en ATS.app → Mostrar contenido del paquete"
    echo "  → Contents → MacOS → crear carpeta 'plugins'"
fi

# 5. Agregar al PATH
SHELL_RC=""
if [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ]; then
    if ! grep -q '.local/bin' "$SHELL_RC" 2>/dev/null; then
        echo "→ Agregando ~/.local/bin al PATH..."
        echo '' >> "$SHELL_RC"
        echo '# G923 FFB' >> "$SHELL_RC"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_RC"
    fi
fi

# 6. Instalar servicio launchctl
echo "→ Instalando servicio de auto-start..."
"$INSTALL_DIR/g923" install-service

echo
echo "╔══════════════════════════════════════════════════╗"
echo "║  ✓ Instalación completa.                        ║"
echo "║                                                  ║"
echo "║  Opciones para usar:                             ║"
echo "║                                                  ║"
echo "║  • App de menú (GUI):                            ║"
echo "║    Abre G923FFB desde ~/.local/bin/               ║"
echo "║                                                  ║"
echo "║  • Terminal (CLI):                               ║"
echo "║    g923 start    ← arranca el daemon             ║"
echo "║    g923 stop     ← detiene el daemon             ║"
echo "║    g923 status   ← muestra estado                ║"
echo "║                                                  ║"
echo "║  • Automático:                                   ║"
echo "║    El daemon arranca solo al iniciar sesión.     ║"
echo "║    Solo abre ATS y maneja.                       ║"
echo "║                                                  ║"
echo "║  Config: ~/.config/g923/g923.toml                ║"
echo "║  Log:    /tmp/g923-ffb.log                       ║"
echo "╚══════════════════════════════════════════════════╝"
echo
read -p "Presiona Enter para cerrar..."
