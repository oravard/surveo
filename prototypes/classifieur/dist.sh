#!/bin/sh
# Prototype 5 (IMPL.md, 5.1) : compile le banc pour Linux et Windows et
# assemble dist/linux et dist/windows (exécutable, ONNX Runtime, modèles).
# Prérequis : export/export.py et export/evaluate.py ont rempli models/.
set -e
cd "$(dirname "$0")"
ORT=1.31.1
mkdir -p runtime
for f in onnxruntime-linux-x64-$ORT.tgz onnxruntime-win-x64-$ORT.zip; do
  [ -f runtime/$f ] || curl -sSL -o runtime/$f https://github.com/microsoft/onnxruntime/releases/download/v$ORT/$f
done
[ -d runtime/onnxruntime-linux-x64-$ORT ] || tar xzf runtime/onnxruntime-linux-x64-$ORT.tgz -C runtime
[ -d runtime/onnxruntime-win-x64-$ORT ] || unzip -qo runtime/onnxruntime-win-x64-$ORT.zip -d runtime

cargo build --release
cargo build --release --target x86_64-pc-windows-gnu

MODELS="model-int8.onnx model-uint8.onnx tokenizer.json labels.json eval.jsonl"
LINUX_ONLY="model-int8.ort model-int8-std.onnx model-fp32.onnx"
rm -rf dist/windows
mkdir -p dist/linux/models dist/windows/models
cp target/release/classifieur dist/linux/
cp runtime/onnxruntime-linux-x64-$ORT/lib/libonnxruntime.so.$ORT dist/linux/libonnxruntime.so
for f in $MODELS $LINUX_ONLY; do ln -sf ../../../models/$f dist/linux/models/$f; done

cp target/x86_64-pc-windows-gnu/release/classifieur.exe dist/windows/
cp runtime/onnxruntime-win-x64-$ORT/lib/onnxruntime.dll dist/windows/
cp mesurer.bat dist/windows/
for f in $MODELS; do cp models/$f dist/windows/models/; done
rm -f dist/classifieur-windows.zip
(cd dist && zip -qr classifieur-windows.zip windows)
ls -la dist/classifieur-windows.zip
