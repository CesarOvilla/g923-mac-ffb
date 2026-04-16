#!/bin/bash
set -e

SRC_APP="$HOME/Library/Application Support/Steam/steamapps/common/American Truck Simulator/American Truck Simulator.app"
DEST="/tmp/ats_with_plugin"
DEST_APP="$DEST/American Truck Simulator.app"
DYLIB="$(cd "$(dirname "$0")" && pwd)/g923_telemetry.dylib"

if [ ! -d "$SRC_APP" ]; then
    echo "✗ No se encontró ATS en: $SRC_APP"
    exit 1
fi

echo "→ Copiando ATS a $DEST (puede tardar ~30s)..."
rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$SRC_APP" "$DEST/"

echo "→ Creando carpeta plugins y copiando dylib..."
mkdir -p "$DEST_APP/Contents/MacOS/plugins"
cp "$DYLIB" "$DEST_APP/Contents/MacOS/plugins/"

echo "→ Quitando firma y quarantine..."
codesign --remove-signature "$DEST_APP" 2>/dev/null || true
xattr -cr "$DEST_APP" 2>/dev/null || true

echo
echo "✓ ATS con plugin listo en: $DEST_APP"
ls -la "$DEST_APP/Contents/MacOS/plugins/"
echo
echo "Para lanzar:"
echo "  open \"$DEST_APP\""
echo
echo "NOTA: Steam no va a reconocer esta copia. Úsala solo para"
echo "verificar que el plugin funciona. Después buscamos una"
echo "solución permanente."
