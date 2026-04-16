#!/bin/bash
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/G923FFB"

swiftc -O -o "$OUT" "$DIR/G923FFB.swift" -framework Cocoa

echo "✓ Compilado: $OUT"
file "$OUT"
