"""Prototype 5 de la phase 2 (IMPL.md, 5.1) : conversion du classifieur
CamemBERT en ONNX, puis quantification dynamique INT8.

Produit dans ../models/ :
  model-fp32.onnx    graphe exporté depuis PyTorch
  model-opt.onnx     graphe optimisé (fusion des couches d'attention)
  model-int8.onnx    poids quantifiés en INT8 sur 7 bits (reduce_range)
  model-uint8.onnx   variante à poids non signés, pour comparaison
  model-int8.ort     idem au format ORT, lisible sans copie des poids
  model-int8-std.onnx  idem, sans fusion : opérateurs ONNX standard
                     seulement, pour un moteur autre qu'ONNX Runtime (tract)
  tokenizer.json, labels.json
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import onnx
import torch
from onnxruntime.quantization import QuantType, quantize_dynamic
from onnxruntime.transformers.optimizer import optimize_model
from transformers import AutoModelForSequenceClassification

SRC = Path("/home/ravard/workspace/trading/IA/sentiment-classifier-camembert-base")
OUT = Path(__file__).resolve().parent.parent / "models"


def export(src: Path, out: Path):
    out.mkdir(exist_ok=True)
    # Attention « eager » : motif que l'optimiseur d'ONNX Runtime sait fusionner.
    model = AutoModelForSequenceClassification.from_pretrained(src, attn_implementation="eager").eval()
    ids = torch.tensor([[5, 100, 200, 300, 6]])
    mask = torch.ones_like(ids)
    fp32 = out / "model-fp32.onnx"
    torch.onnx.export(
        model,
        (ids, mask),
        fp32,
        input_names=["input_ids", "attention_mask"],
        output_names=["logits"],
        dynamic_axes={"input_ids": {0: "batch", 1: "seq"}, "attention_mask": {0: "batch", 1: "seq"}},
        opset_version=17,
        dynamo=False,
    )

    opt = optimize_model(str(fp32), model_type="bert", num_heads=12, hidden_size=768)
    opt.save_model_to_file(str(out / "model-opt.onnx"))

    # Les opérateurs fusionnés (domaine com.microsoft) échappent à
    # l'inférence de types d'ONNX.
    types = {"DefaultTensorType": onnx.TensorProto.FLOAT}
    # Sans VNNI (AVX2 seul, par exemple AMD Zen 3), ONNX Runtime multiplie
    # octets non signés et signés par vpmaddubsw, qui sature sur 16 bits :
    # avec des poids sur 8 bits, le modèle ne reconnaît plus rien (IMPL.md,
    # 5.1). Des poids sur 7 bits évitent la saturation.
    quantize_dynamic(
        out / "model-opt.onnx",
        out / "model-int8.onnx",
        weight_type=QuantType.QInt8,
        per_channel=True,
        reduce_range=True,
        extra_options=types,
    )
    quantize_dynamic(
        out / "model-opt.onnx",
        out / "model-uint8.onnx",
        weight_type=QuantType.QUInt8,
        per_channel=True,
        extra_options=types,
    )

    with tempfile.TemporaryDirectory() as tmp:
        shutil.copy(out / "model-int8.onnx", tmp)
        subprocess.run(
            [sys.executable, "-m", "onnxruntime.tools.convert_onnx_models_to_ort", tmp, "--optimization_style", "Fixed"],
            check=True,
        )
        shutil.copy(Path(tmp) / "model-int8.ort", out)

    std = out / "model-int8-std.onnx"
    quantize_dynamic(fp32, std, weight_type=QuantType.QInt8, per_channel=True)
    # Une phrase à la fois : lot fixé à 1. tract ne sait pas calculer la
    # forme d'un produit de deux dimensions symboliques (batch × seq).
    m = onnx.load(std)
    del m.graph.value_info[:]
    for v in [*m.graph.input, *m.graph.output]:
        v.type.tensor_type.shape.dim[0].dim_value = 1
    onnx.save(m, std)

    shutil.copy(src / "tokenizer.json", out / "tokenizer.json")
    labels = [model.config.id2label[i] for i in range(model.config.num_labels)]
    (out / "labels.json").write_text(json.dumps(labels, ensure_ascii=False))
    for f in sorted([*out.glob("*.onnx"), *out.glob("*.ort")]):
        print(f"{f.name:20} {f.stat().st_size / 1e6:7.1f} Mo")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--src", type=Path, default=SRC)
    p.add_argument("--out", type=Path, default=OUT)
    a = p.parse_args()
    export(a.src, a.out)
