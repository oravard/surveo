//! Fichiers de l'utilisateur : dossier Documents, ouverture par
//! l'application habituelle, noms sûrs qui n'écrasent rien, empreinte du
//! contenu (SPEC.md, chap. 3 ; IMPL.md, 3.1).

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Dossier Documents de l'utilisateur, ou à défaut son dossier personnel.
pub fn documents_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        // Le dossier peut avoir été déplacé (OneDrive, partage réseau).
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders")
            .and_then(|k| k.get_value::<String, _>("Personal"));
        match key {
            Ok(v) => PathBuf::from(expand_env(&v)),
            Err(_) => home.join("Documents"),
        }
    }
    #[cfg(not(windows))]
    {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
        let from_xdg = std::fs::read_to_string(config.join("user-dirs.dirs")).ok().and_then(|t| {
            t.lines().find_map(|l| {
                let v = l.trim().strip_prefix("XDG_DOCUMENTS_DIR=")?.trim_matches('"');
                Some(PathBuf::from(v.replace("$HOME", &home.to_string_lossy())))
            })
        });
        match from_xdg {
            // « $HOME/ » seul désigne le dossier personnel.
            Some(p) if p != home && p.is_dir() => p,
            _ if home.join("Documents").is_dir() => home.join("Documents"),
            _ => home,
        }
    }
}

/// Remplace `%VARIABLE%` par sa valeur (chemins du registre Windows).
#[cfg_attr(not(windows), allow(dead_code))]
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut parts = s.split('%');
    out.push_str(parts.next().unwrap_or(""));
    let mut var = true;
    for p in parts {
        if var {
            match std::env::var(p) {
                Ok(v) if !p.is_empty() => out.push_str(&v),
                _ => {
                    out.push('%');
                    out.push_str(p);
                    out.push('%');
                }
            }
        } else {
            out.push_str(p);
        }
        var = !var;
    }
    out
}

/// Ouvre un fichier ou un dossier avec l'application habituelle du poste.
pub fn open_path(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        // SAFETY : chaînes terminées par un zéro, vivantes pendant l'appel.
        let r = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        anyhow::ensure!(r as isize > 32, "ouverture de {} impossible (code {})", path.display(), r as isize);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("ouverture de {}", path.display()))?;
        Ok(())
    }
}

/// Nom de fichier sûr sur tous les systèmes : sans dossier, sans
/// caractère interdit, sans point ni espace final (Windows).
pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let s: String = base.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3"];
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    match s.as_str() {
        "" | "." | ".." => "fichier".into(),
        _ if reserved.contains(&stem.as_str()) => format!("_{s}"),
        _ => s.chars().take(150).collect(),
    }
}

/// Sépare « rapport.final.odt » en « rapport.final » et « .odt ».
pub fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// Chemin libre pour `name` dans `dir` : un fichier existant n'est
/// jamais écrasé, le nouveau devient « nom (2).ext » (DISTRIB-2).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let name = safe_file_name(name);
    let first = dir.join(&name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = split_ext(&name);
    (2..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).expect("nom libre")
}

/// Empreinte SHA-256 d'un contenu, en hexadécimal.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Calcul d'empreinte au fil de l'eau, pour un fichier reçu par morceaux.
#[derive(Clone, Default)]
pub struct Hasher(Sha256);

impl Hasher {
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    pub fn finish(self) -> String {
        hex(&self.0.finalize())
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Copie `src` vers `dst` en calculant son empreinte ; rend la taille et
/// l'empreinte. Le fichier d'origine reste en place (RENDU-1).
pub fn copy_hashed(src: &Path, dst: &Path) -> Result<(u64, String)> {
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut from = std::fs::File::open(src).with_context(|| format!("lecture de {}", src.display()))?;
    let mut to = std::fs::File::create(dst).with_context(|| format!("écriture de {}", dst.display()))?;
    let mut h = Hasher::default();
    let mut buf = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        to.write_all(&buf[..n])?;
        size += n as u64;
    }
    to.sync_all()?;
    Ok((size, h.finish()))
}

/// Lit au plus `len` octets de `path` à partir de `offset`.
pub fn read_at(path: &Path, offset: u64, len: usize) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(path).with_context(|| format!("lecture de {}", path.display()))?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(len);
    f.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Déplace un fichier, en le copiant s'il change de disque.
pub fn move_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to).with_context(|| format!("écriture de {}", to.display()))?;
        let _ = std::fs::remove_file(from);
    }
    Ok(())
}

/// « 1,4 Mo », « 820 Ko ».
pub fn size_text(bytes: u64) -> String {
    const K: f64 = 1000.0;
    let b = bytes as f64;
    let (v, unit) = if b >= K * K * K {
        (b / (K * K * K), "Go")
    } else if b >= K * K {
        (b / (K * K), "Mo")
    } else if b >= K {
        (b / K, "Ko")
    } else {
        return format!("{bytes} o");
    };
    let s = if v < 10.0 { format!("{v:.1}") } else { format!("{v:.0}") };
    format!("{} {unit}", s.replace('.', ",").trim_end_matches(",0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_safe_and_never_overwrite() {
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name(r"C:\Users\x\rapport: v2?.odt "), "rapport_ v2_.odt");
        assert_eq!(safe_file_name("..."), "fichier");
        assert_eq!(safe_file_name("con.txt"), "_con.txt");
        let dir = std::env::temp_dir().join(format!("surveo-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cours.pdf"), b"de l'eleve").unwrap();
        assert_eq!(unique_path(&dir, "cours.pdf"), dir.join("cours (2).pdf"));
        assert_eq!(unique_path(&dir, "autre.pdf"), dir.join("autre.pdf"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copies_are_hashed_and_sizes_read_well() {
        let dir = std::env::temp_dir().join(format!("surveo-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a"), b"abc").unwrap();
        let (n, h) = copy_hashed(&dir.join("a"), &dir.join("b")).unwrap();
        assert_eq!((n, h.as_str()), (3, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert_eq!(read_at(&dir.join("b"), 1, 10).unwrap(), b"bc");
        assert_eq!(sha256_hex(b"abc"), h);
        assert_eq!((size_text(820), size_text(1_400_000), size_text(50_000_000)), ("820 o".into(), "1,4 Mo".into(), "50 Mo".into()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
