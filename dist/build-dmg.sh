#!/bin/bash
# Construye el .dmg distribuible con todos los componentes.
set -e

PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
STAGING="$PROJECT/dist/staging"
DMG_NAME="G923-FFB-macOS"
DMG_PATH="$PROJECT/dist/$DMG_NAME.dmg"

echo "→ Compilando binarios Rust (release)..."
cd "$PROJECT"
cargo build --release --bin g923 --bin g923-daemon

echo "→ Compilando menu bar app..."
bash "$PROJECT/app/build.sh"

echo "→ Compilando plugin de telemetría..."
bash "$PROJECT/plugin/build.sh"

echo "→ Preparando staging..."
rm -rf "$STAGING"
mkdir -p "$STAGING"

# Copiar binarios
cp "$PROJECT/target/release/g923-daemon" "$STAGING/"
cp "$PROJECT/target/release/g923" "$STAGING/"
cp "$PROJECT/app/G923FFB" "$STAGING/"
cp "$PROJECT/plugin/g923_telemetry.dylib" "$STAGING/"

# Config por defecto
cp "$PROJECT/g923.toml" "$STAGING/"

# Instalador
cp "$PROJECT/dist/Instalar.command" "$STAGING/"
chmod +x "$STAGING/Instalar.command"

# README breve
cat > "$STAGING/LÉEME.txt" << 'EOF'
G923 FFB para macOS
====================

Force Feedback para Logitech G923 Xbox en Mac Apple Silicon.

INSTALACIÓN:
  Doble-click en "Instalar.command" — hace todo automático.

USO:
  1. Abre ATS/ETS2 desde Steam
  2. Maneja — el FFB se activa solo

CONFIGURACIÓN:
  Edita ~/.config/g923/g923.toml para ajustar intensidades.
  Los cambios se aplican automáticamente sin reiniciar.

SOPORTE:
  GitHub: https://github.com/tu-usuario/g923-mac-ffb
EOF

echo "→ Creando $DMG_NAME.dmg..."
rm -f "$DMG_PATH"
hdiutil create -volname "$DMG_NAME" -srcfolder "$STAGING" \
  -ov -format UDZO "$DMG_PATH"

rm -rf "$STAGING"

echo
echo "✓ DMG creado: $DMG_PATH"
echo "  Tamaño: $(du -h "$DMG_PATH" | cut -f1)"
