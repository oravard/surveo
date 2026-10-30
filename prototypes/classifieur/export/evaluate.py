"""Compare le modèle PyTorch d'origine et ses conversions ONNX sur la partie
de validation du corpus d'entraînement, reconstituée à l'identique
(sentiment_classifier.py : 20 %, random_state=42, stratifiée).

Écrit aussi ../models/eval.jsonl (texte, étiquette attendue, prédiction
PyTorch) pour que le banc Rust vérifie sa propre chaîne.
"""

import argparse
import json
import time
from pathlib import Path

import numpy as np
import onnxruntime as ort
import pandas as pd
import torch
from sklearn.metrics import accuracy_score, f1_score, recall_score
from sklearn.model_selection import train_test_split
from transformers import AutoModelForSequenceClassification, AutoTokenizer

from export import OUT, SRC

CORPUS = Path("/home/ravard/workspace/trading/IA/sentiment/clean_corpus.csv")


def eval_split():
    df = pd.read_csv(CORPUS)[["text", "sentiment"]].dropna()
    counts = df["sentiment"].value_counts()
    df = df[~df["sentiment"].isin(counts[counts < 2].index)]
    label2id = {l: i for i, l in enumerate(df["sentiment"].unique())}
    df["labels"] = df["sentiment"].map(label2id)
    _, ev = train_test_split(df, test_size=0.2, random_state=42, stratify=df["labels"])
    return ev, label2id


def report(name, y, pred, ref, times, labels):
    rec = recall_score(y, pred, average=None, labels=range(len(labels)))
    print(
        f"{name:10} exactitude {accuracy_score(y, pred):.4f}  F1 macro {f1_score(y, pred, average='macro'):.4f}"
        f"  accord/PyTorch {np.mean(np.array(pred) == np.array(ref)):.4f}"
        f"  médiane {np.median(times):5.1f} ms  p95 {np.percentile(times, 95):5.1f} ms"
    )
    print("           rappel " + "  ".join(f"{l} {r:.4f}" for l, r in zip(labels, rec)))


def main(limit, threads):
    ev, label2id = eval_split()
    if limit:
        ev = ev.sample(n=limit, random_state=0)
    tok = AutoTokenizer.from_pretrained(SRC)
    model = AutoModelForSequenceClassification.from_pretrained(SRC).eval()
    labels = [model.config.id2label[i] for i in range(model.config.num_labels)]
    # Le corpus et le modèle doivent s'accorder sur la numérotation.
    assert all(label2id[l] == i for i, l in enumerate(labels)), (label2id, labels)
    texts, y = ev["text"].tolist(), ev["labels"].tolist()
    torch.set_num_threads(threads)

    def run(fn):
        preds, times = [], []
        for t in texts:
            enc = tok(t, truncation=True, max_length=128, return_tensors="np")
            t0 = time.perf_counter()
            preds.append(int(np.argmax(fn(enc))))
            times.append((time.perf_counter() - t0) * 1000)
        return preds, times

    with torch.no_grad():
        ref, times = run(lambda e: model(**{k: torch.from_numpy(v) for k, v in e.items()}).logits.numpy())
    report("PyTorch", y, ref, ref, times, labels)

    opts = ort.SessionOptions()
    opts.intra_op_num_threads = threads
    for name in ["model-fp32", "model-opt", "model-int8", "model-uint8"]:
        s = ort.InferenceSession(str(OUT / f"{name}.onnx"), opts, providers=["CPUExecutionProvider"])
        pred, times = run(lambda e: s.run(None, {"input_ids": e["input_ids"], "attention_mask": e["attention_mask"]})[0])
        report(name, y, pred, ref, times, labels)

    with open(OUT / "eval.jsonl", "w") as f:
        for t, l, r in zip(texts, y, ref):
            f.write(json.dumps({"text": t, "label": l, "torch": r}, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--limit", type=int, default=0, help="nombre de phrases (0 : toute la validation)")
    p.add_argument("--threads", type=int, default=1)
    a = p.parse_args()
    main(a.limit, a.threads)
