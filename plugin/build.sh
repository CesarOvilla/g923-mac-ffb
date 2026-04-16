#!/bin/bash
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/g923_telemetry.dylib"

clang -arch x86_64 \
      -shared \
      -fvisibility=hidden \
      -O2 \
      -framework IOKit \
      -framework CoreFoundation \
      -o "$OUT" \
      "$DIR/g923_telemetry.c"

echo "✓ Compilado: $OUT"
file "$OUT"
