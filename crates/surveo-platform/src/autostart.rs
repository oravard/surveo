//! Démarrage à l'ouverture de session, sans droits d'administration
//! (SPEC.md, INST-1, DEMAR-1 ; IMPL.md, 2.1) : clé `Run` de l'utilisateur
//! sous Windows, fichier dans `~/.config/autostart` sous Linux.

use anyhow::Result;
use std::path::Path;

/// Inscrit `exe` au démarrage de la session, sous le nom `name`.
pub fn register(name: &str, label: &str, exe: &Path) -> Result<()> {
    imp::register(name, label, exe)
}

pub fn unregister(name: &str) -> Result<()> {
    imp::unregister(name)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn register(name: &str, _label: &str, exe: &Path) -> Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
        key.set_value(name, &format!("\"{}\"", exe.display()))?;
        Ok(())
    }

    pub fn unregister(name: &str) -> Result<()> {
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
            let _ = key.delete_value(name);
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str) -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
        base.join("autostart").join(format!("{name}.desktop"))
    }

    pub fn register(name: &str, label: &str, exe: &Path) -> Result<()> {
        let f = file(name);
        std::fs::create_dir_all(f.parent().expect("dossier autostart"))?;
        std::fs::write(
            &f,
            format!(
                "[Desktop Entry]\nType=Application\nName={label}\nExec=\"{}\"\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
                exe.display()
            ),
        )?;
        Ok(())
    }

    pub fn unregister(name: &str) -> Result<()> {
        let _ = std::fs::remove_file(file(name));
        Ok(())
    }
}
