//! Prototype 5 de la phase 2 (IMPL.md, 5.1) : le classifieur CamemBERT,
//! converti en ONNX et quantifié, exécuté depuis Rust sur un poste élève.
//!
//! Sans texte en argument, rejoue la partie de validation (eval.jsonl,
//! produite par export/evaluate.py) et mesure temps, mémoire et accord
//! avec PyTorch. Avec un texte, affiche les scores de chaque classe.
//!
//! Deux moteurs : ONNX Runtime (par défaut), chargé depuis sa bibliothèque,
//! ou tract (`--backend tract`), écrit en Rust et compilé dans l'exécutable.
//!
//! Disposition attendue à côté de l'exécutable :
//!   onnxruntime.dll ou libonnxruntime.so (moteur ONNX Runtime seulement)
//!   models/model-int8.onnx (ONNX Runtime), models/model-int8-std.onnx (tract),
//!   models/tokenizer.json, models/labels.json, models/eval.jsonl

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use ort::environment::Environment;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
#[cfg(feature = "tract")]
use std::sync::Arc;
#[cfg(feature = "tract")]
use tract_onnx::prelude::{tvec, IntoTValue, TypedRunnableModel};

const MAX_TOKENS: usize = 128;

struct Args {
    models: PathBuf,
    tract: bool,
    model: Option<String>,
    ort: PathBuf,
    threads: usize,
    limit: usize,
    /// Réglages de session d'ONNX Runtime (clé=valeur), pour les essais.
    config: Vec<(String, String)>,
    texts: Vec<String>,
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default()
}

fn parse_args() -> Result<Args> {
    let lib = if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.so" };
    let mut a = Args {
        models: exe_dir().join("models"),
        tract: false,
        model: None,
        ort: exe_dir().join(lib),
        threads: 1,
        limit: 0,
        config: vec![],
        texts: vec![],
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or_else(|| anyhow!("valeur manquante après {arg}"));
        match arg.as_str() {
            "--models" => a.models = val()?.into(),
            "--model" => a.model = Some(val()?),
            "--backend" => match val()?.as_str() {
                "ort" => a.tract = false,
                "tract" => a.tract = true,
                other => return Err(anyhow!("moteur inconnu : {other} (ort ou tract)")),
            },
            "--ort" => a.ort = val()?.into(),
            "--threads" => a.threads = val()?.parse()?,
            "--limit" => a.limit = val()?.parse()?,
            "--config" => {
                let kv = val()?;
                let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("--config attend clé=valeur"))?;
                a.config.push((k.into(), v.into()));
            }
            "-h" | "--help" => {
                println!("classifieur [--backend ort|tract] [--models DIR] [--model FICHIER.onnx] [--ort LIB] [--threads N] [--limit N] [--config CLÉ=VALEUR] [TEXTE...]");
                std::process::exit(0);
            }
            _ => a.texts.push(arg),
        }
    }
    Ok(a)
}

impl Args {
    fn model(&self) -> &str {
        self.model.as_deref().unwrap_or(if self.tract { "model-int8-std.onnx" } else { "model-int8.onnx" })
    }
}

/// Mémoire résidente actuelle et maximale, en Mo.
#[cfg(target_os = "linux")]
fn memory() -> (f64, f64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .map_or(0.0, |kb| kb / 1024.0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

#[cfg(windows)]
fn memory() -> (f64, f64) {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    c.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    let mb = |b: usize| b as f64 / (1024.0 * 1024.0);
    (mb(c.WorkingSetSize), mb(c.PeakWorkingSetSize))
}

enum Engine {
    Ort(Session),
    #[cfg(feature = "tract")]
    Tract(Arc<TypedRunnableModel>),
}

struct Classifier {
    engine: Engine,
    tokenizer: Tokenizer,
}

impl Classifier {
    fn load(env: Option<&Environment>, a: &Args) -> Result<Self> {
        let models = &a.models;
        let mut tokenizer = Tokenizer::from_file(models.join("tokenizer.json")).map_err(|e| anyhow!("{e}"))?;
        // Le fichier enregistré à l'entraînement complète à 128 jetons :
        // inutile pour une phrase seule, et trois fois plus coûteux.
        tokenizer.with_padding(None);
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams { max_length: MAX_TOKENS, ..Default::default() }))
            .map_err(|e| anyhow!("{e}"))?;
        let path = models.join(a.model());
        let engine = match env {
            Some(env) => Engine::Ort(open_session(env, &path, a.threads, &a.config)?),
            #[cfg(feature = "tract")]
            None => Engine::Tract(open_tract(&path)?),
            #[cfg(not(feature = "tract"))]
            None => return Err(anyhow!("moteur tract absent de cette construction")),
        };
        Ok(Self { engine, tokenizer })
    }

    /// Probabilités de chaque classe, et nombre de jetons.
    fn classify(&mut self, text: &str) -> Result<(Vec<f32>, usize)> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| anyhow!("{e}"))?;
        let ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        let n = ids.len();
        let mask = vec![1i64; n];
        let logits: Vec<f32> = match &mut self.engine {
            Engine::Ort(session) => {
                let outputs = session.run(ort::inputs![
                    "input_ids" => Tensor::from_array(([1usize, n], ids))?,
                    "attention_mask" => Tensor::from_array(([1usize, n], mask))?,
                ])?;
                outputs["logits"].try_extract_tensor::<f32>()?.1.to_vec()
            }
            #[cfg(feature = "tract")]
            Engine::Tract(model) => {
                let ids = tract_onnx::prelude::Tensor::from_shape(&[1, n], &ids)?;
                let mask = tract_onnx::prelude::Tensor::from_shape(&[1, n], &mask)?;
                let out = model.run(tvec!(ids.into_tvalue(), mask.into_tvalue()))?;
                out[0].to_plain_array_view::<f32>()?.iter().copied().collect()
            }
        };
        let max = logits.iter().cloned().fold(f32::MIN, f32::max);
        let exp: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
        let sum: f32 = exp.iter().sum();
        Ok((exp.iter().map(|e| e / sum).collect(), n))
    }
}

fn open_session(env: &Environment, path: &Path, threads: usize, config: &[(String, String)]) -> ort::Result<Session> {
    let mut b = Session::builder(env)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?;
    for (k, v) in config {
        b = b.with_config_entry(k, v)?;
    }
    if path.extension().is_some_and(|e| e == "ort") {
        // Format .ort : ONNX Runtime lit les poids dans le tampon du fichier
        // sans les recopier. Le tampon doit vivre autant que la session.
        let bytes: &'static [u8] = Box::leak(std::fs::read(path).map_err(ort::Error::wrap)?.into_boxed_slice());
        b = b
            .with_config_entry("session.use_ort_model_bytes_directly", "1")?
            .with_config_entry("session.use_ort_model_bytes_for_initializers", "1")?;
        return b.commit_from_memory(bytes);
    }
    b.commit_from_file(path)
}

#[cfg(feature = "tract")]
/// tract ne connaît pas les opérateurs fusionnés d'ONNX Runtime : il lit le
/// graphe quantifié sans fusion (export/export.py, model-int8-std.onnx).
fn open_tract(path: &Path) -> Result<Arc<TypedRunnableModel>> {
    use tract_onnx::prelude::*;
    let mut model = tract_onnx::onnx().model_for_path(path)?;
    // Même symbole que l'axe dynamique de l'export ; lot fixé à 1.
    let fact = i64::fact(&[1.to_dim(), model.sym("seq").to_dim()]);
    model.set_input_fact(0, fact.clone().into())?;
    model.set_input_fact(1, fact.into())?;
    Ok(model.into_optimized()?.into_runnable()?)
}

fn argmax(p: &[f32]) -> usize {
    p.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i)
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

#[derive(serde::Deserialize)]
struct Sample {
    text: String,
    label: usize,
    torch: usize,
}

fn main() -> Result<()> {
    let a = parse_args()?;
    let (rss0, _) = memory();
    let t0 = Instant::now();
    let env = match a.tract {
        true => None,
        false => Some(
            ort::init_from(&a.ort).with_context(|| format!("ONNX Runtime introuvable : {}", a.ort.display()))?.build()?,
        ),
    };
    let labels: Vec<String> = serde_json::from_str(&std::fs::read_to_string(a.models.join("labels.json"))?)?;
    let mut clf = Classifier::load(env.as_ref(), &a)?;
    let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let (rss_loaded, _) = memory();
    println!(
        "{} ({}) chargé en {load_ms:.0} ms, {} fil(s) ; mémoire {rss0:.0} Mo au départ, {rss_loaded:.0} Mo après chargement",
        a.model(),
        if a.tract { "tract" } else { "ONNX Runtime" },
        if a.tract { 1 } else { a.threads }
    );

    if !a.texts.is_empty() {
        for text in &a.texts {
            let t = Instant::now();
            let (p, n) = clf.classify(text)?;
            let scores: Vec<String> = labels.iter().zip(&p).map(|(l, s)| format!("{l} {s:.3}")).collect();
            println!(
                "{:<12} {:5.1} ms {n:3} jetons  [{}]  {text}",
                labels[argmax(&p)],
                t.elapsed().as_secs_f64() * 1000.0,
                scores.join(", ")
            );
        }
        return Ok(());
    }

    let file = std::fs::File::open(a.models.join("eval.jsonl")).context("eval.jsonl")?;
    let mut samples: Vec<Sample> =
        std::io::BufReader::new(file).lines().map(|l| Ok(serde_json::from_str(&l?)?)).collect::<Result<_>>()?;
    if a.limit > 0 {
        samples.truncate(a.limit);
    }
    // Première exécution à part : ONNX Runtime y prépare ses tampons.
    let t = Instant::now();
    clf.classify(&samples[0].text)?;
    let first_ms = t.elapsed().as_secs_f64() * 1000.0;

    let k = labels.len();
    let (mut correct, mut agree) = (0usize, 0usize);
    let (mut hits, mut totals) = (vec![0usize; k], vec![0usize; k]);
    let mut times = Vec::with_capacity(samples.len());
    let mut tokens = 0usize;
    let cpu0 = Instant::now();
    for s in &samples {
        let t = Instant::now();
        let (p, n) = clf.classify(&s.text)?;
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        tokens += n;
        let pred = argmax(&p);
        correct += (pred == s.label) as usize;
        agree += (pred == s.torch) as usize;
        totals[s.label] += 1;
        hits[s.label] += (pred == s.label) as usize;
    }
    let wall = cpu0.elapsed().as_secs_f64();
    times.sort_by(f64::total_cmp);
    let (rss, peak) = memory();
    let n = samples.len() as f64;
    println!("{} phrases, {:.1} jetons en moyenne, première phrase {first_ms:.1} ms", samples.len(), tokens as f64 / n);
    println!(
        "temps par phrase : médiane {:.1} ms, p95 {:.1} ms, p99 {:.1} ms, max {:.1} ms ; {:.0} phrases/s",
        percentile(&times, 0.5),
        percentile(&times, 0.95),
        percentile(&times, 0.99),
        times[times.len() - 1],
        n / wall
    );
    println!("mémoire : {rss:.0} Mo à la fin, pic {peak:.0} Mo");
    println!("exactitude {:.4}, accord avec PyTorch {:.4}", correct as f64 / n, agree as f64 / n);
    let recall: Vec<String> =
        labels.iter().enumerate().map(|(i, l)| format!("{l} {:.4}", hits[i] as f64 / totals[i].max(1) as f64)).collect();
    println!("rappel : {}", recall.join(", "));
    Ok(())
}
