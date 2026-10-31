#!/usr/bin/env bash
# Construit une version opérationnelle de Surveo (IMPL.md, « Ordre de
# réalisation (phase 4) ») : les interfaces, puis l'agent et la console
# (relais intégré) pour Linux et pour Windows, compilés depuis Linux.
#
# Résultat dans dist/ : surveo-<version>-linux.tar.gz et
# surveo-<version>-windows.zip, avec leurs empreintes SHA-256.
#
# Usage : scripts/build.sh [--linux-seul]
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
WIN=x86_64-pc-windows-gnu

echo "== Interfaces"
(cd ui && npm ci --no-audit --no-fund --silent && npm run --silent build)

echo "== Linux"
cargo build --release -p surveo-agent -p surveo-console
if [[ "${1:-}" != "--linux-seul" ]]; then
  echo "== Windows"
  cargo build --release --target "$WIN" -p surveo-agent -p surveo-console
fi

rm -rf dist && mkdir -p dist
assemble() { # $1 = système, puis les fichiers à livrer
  local os=$1; shift
  local dir="dist/surveo-$VERSION-$os"
  mkdir -p "$dir"
  cp "$@" "$dir/"
  sed "s/{VERSION}/$VERSION/" scripts/LISEZMOI.txt > "$dir/LISEZMOI.txt"
  (cd "$dir" && sha256sum -- * > SHA256SUMS)
}

assemble linux target/release/surveo-agent target/release/surveo-console
tar -C dist -czf "dist/surveo-$VERSION-linux.tar.gz" "surveo-$VERSION-linux"

if [[ "${1:-}" != "--linux-seul" ]]; then
  assemble windows target/$WIN/release/surveo-agent.exe target/$WIN/release/surveo-console.exe \
    target/$WIN/release/WebView2Loader.dll
  (cd dist && rm -f "surveo-$VERSION-windows.zip" && zip -qr "surveo-$VERSION-windows.zip" "surveo-$VERSION-windows")
fi

(cd dist && sha256sum -- *.tar.gz *.zip 2>/dev/null > SHA256SUMS || true)
ls -l dist
