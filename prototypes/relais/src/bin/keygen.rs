//! Génère un établissement de démonstration et les identités de ses
//! membres (relais, console, agents), plus un second établissement pour
//! vérifier qu'un membre étranger est refusé (SPEC.md, CONF-3).
//!
//! Usage : keygen [--out keys] [--etab lycee-demo] [--agents 300]

use anyhow::Result;
use std::path::PathBuf;
use surveo_proto_relais::{arg, save_json, Establishment, Role};

fn main() -> Result<()> {
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| "keys".into()));
    let etab_id = arg("--etab").unwrap_or_else(|| "lycee-demo".into());
    let n: usize = arg("--agents").map(|s| s.parse()).transpose()?.unwrap_or(300);

    std::fs::create_dir_all(out.join("autre"))?;
    let etab = Establishment::generate(&etab_id);
    save_json(&out.join("etab.json"), &etab.public())?;
    save_json(&out.join("relay.json"), &etab.issue("relais-b204", Role::Relay))?;
    save_json(&out.join("console.json"), &etab.issue("console-durand", Role::Console))?;
    let agents: Vec<_> = (1..=n).map(|i| etab.issue(&format!("poste-{i:03}"), Role::Agent)).collect();
    save_json(&out.join("agents.json"), &agents)?;

    let other = Establishment::generate("autre-lycee");
    let foreign: Vec<_> = (1..=3).map(|i| other.issue(&format!("intrus-{i}"), Role::Agent)).collect();
    save_json(&out.join("autre/agents.json"), &foreign)?;

    println!("Établissement « {etab_id} » : relais, console et {n} agents dans {}", out.display());
    println!("Établissement « autre-lycee » : 3 agents dans {}", out.join("autre").display());
    Ok(())
}
