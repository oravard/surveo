//! Réglage du proxy de l'utilisateur, sans droits d'administration
//! (SPEC.md, WEB-1, MODE-5 ; IMPL.md, 2.1) : API WinINet sous Windows,
//! clés `org.gnome.system.proxy` par `gsettings` sous Linux.
//!
//! L'agent pose un script de configuration automatique (PAC) qu'il sert
//! lui-même : si l'agent s'arrête, le script devient injoignable et le
//! navigateur se connecte directement, si bien que le filtrage cesse avec
//! l'agent (MODE-5). Le réglage d'origine est relevé en entier et
//! restitué tel quel (RESTR-4).

use anyhow::Result;
use std::sync::Mutex;

/// Accès au réglage du proxy de l'utilisateur.
pub trait UserProxy: Send + Sync {
    /// Relève le réglage en place, sous une forme à restituer telle quelle.
    fn snapshot(&self) -> Result<String>;
    /// Pose le script de configuration automatique `url`.
    fn set_pac(&self, url: &str) -> Result<()>;
    fn restore(&self, snapshot: &str) -> Result<()>;
    /// Le réglage en place est-il encore le script `url` ?
    fn is_pac(&self, url: &str) -> bool;
}

/// Le réglage réel de la session.
pub struct SystemProxy;

impl UserProxy for SystemProxy {
    fn snapshot(&self) -> Result<String> {
        Ok(serde_json::to_string(&imp::read()?)?)
    }

    fn set_pac(&self, url: &str) -> Result<()> {
        imp::set_pac(url)
    }

    fn restore(&self, snapshot: &str) -> Result<()> {
        imp::write(&serde_json::from_str(snapshot)?)
    }

    fn is_pac(&self, url: &str) -> bool {
        imp::is_pac(url).unwrap_or(false)
    }
}

/// Réglage simulé, pour les essais : il ne touche pas à la session.
#[derive(Default)]
pub struct MemoryProxy {
    pub value: Mutex<String>,
}

impl UserProxy for MemoryProxy {
    fn snapshot(&self) -> Result<String> {
        Ok(self.value.lock().unwrap().clone())
    }

    fn set_pac(&self, url: &str) -> Result<()> {
        *self.value.lock().unwrap() = format!("pac {url}");
        Ok(())
    }

    fn restore(&self, snapshot: &str) -> Result<()> {
        *self.value.lock().unwrap() = snapshot.into();
        Ok(())
    }

    fn is_pac(&self, url: &str) -> bool {
        *self.value.lock().unwrap() == format!("pac {url}")
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::{bail, Context, Result};
    use serde::{Deserialize, Serialize};
    use std::process::Command;

    const KEYS: [(&str, &str); 7] = [
        ("org.gnome.system.proxy", "mode"),
        ("org.gnome.system.proxy", "autoconfig-url"),
        ("org.gnome.system.proxy", "ignore-hosts"),
        ("org.gnome.system.proxy.http", "host"),
        ("org.gnome.system.proxy.http", "port"),
        ("org.gnome.system.proxy.https", "host"),
        ("org.gnome.system.proxy.https", "port"),
    ];

    /// Valeurs des clés, en texte GVariant, restituées telles quelles.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct Snapshot {
        values: Vec<String>,
    }

    fn gsettings(args: &[&str]) -> Result<String> {
        let out = Command::new("gsettings").args(args).output().context("réglage du proxy introuvable (gsettings)")?;
        if !out.status.success() {
            bail!("réglage du proxy indisponible : {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn read() -> Result<Snapshot> {
        Ok(Snapshot { values: KEYS.iter().map(|(s, k)| gsettings(&["get", s, k])).collect::<Result<_>>()? })
    }

    pub fn write(s: &Snapshot) -> Result<()> {
        // Le mode en dernier, pour ne jamais activer un réglage à moitié écrit.
        for i in [1, 2, 3, 4, 5, 6, 0] {
            let (schema, key) = KEYS[i];
            gsettings(&["set", schema, key, &s.values[i]])?;
        }
        Ok(())
    }

    /// Seules l'adresse du script et le mode changent.
    pub fn set_pac(url: &str) -> Result<()> {
        gsettings(&["set", KEYS[1].0, KEYS[1].1, &format!("'{url}'")])?;
        gsettings(&["set", KEYS[0].0, KEYS[0].1, "'auto'"])?;
        Ok(())
    }

    /// Vérifiée toutes les deux secondes : seules deux clés sont lues.
    pub fn is_pac(url: &str) -> Result<bool> {
        Ok(gsettings(&["get", KEYS[0].0, KEYS[0].1])? == "'auto'" && gsettings(&["get", KEYS[1].0, KEYS[1].1])? == format!("'{url}'"))
    }
}

#[cfg(windows)]
mod imp {
    use anyhow::{bail, Result};
    use serde::{Deserialize, Serialize};
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Networking::WinInet::*;

    /// Réglage de proxy de l'utilisateur, tel que WinINet le décrit.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct Snapshot {
        flags: u32,
        server: Option<String>,
        bypass: Option<String>,
        autoconfig_url: Option<String>,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn take_wide(p: *mut u16) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        GlobalFree(p as *mut c_void);
        Some(s)
    }

    pub fn read() -> Result<Snapshot> {
        let mut opts: [INTERNET_PER_CONN_OPTIONW; 4] = unsafe { std::mem::zeroed() };
        opts[0].dwOption = INTERNET_PER_CONN_FLAGS;
        opts[1].dwOption = INTERNET_PER_CONN_PROXY_SERVER;
        opts[2].dwOption = INTERNET_PER_CONN_PROXY_BYPASS;
        opts[3].dwOption = INTERNET_PER_CONN_AUTOCONFIG_URL;
        let mut list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            pszConnection: std::ptr::null_mut(),
            dwOptionCount: 4,
            dwOptionError: 0,
            pOptions: opts.as_mut_ptr(),
        };
        let mut size = list.dwSize;
        let ok = unsafe {
            InternetQueryOptionW(std::ptr::null(), INTERNET_OPTION_PER_CONNECTION_OPTION, &mut list as *mut _ as *mut c_void, &mut size)
        };
        if ok == 0 {
            bail!("lecture du proxy impossible : {}", std::io::Error::last_os_error());
        }
        unsafe {
            Ok(Snapshot {
                flags: opts[0].Value.dwValue,
                server: take_wide(opts[1].Value.pszValue),
                bypass: take_wide(opts[2].Value.pszValue),
                autoconfig_url: take_wide(opts[3].Value.pszValue),
            })
        }
    }

    pub fn write(s: &Snapshot) -> Result<()> {
        let (mut server, mut bypass, mut url) =
            (s.server.as_deref().map(wide), s.bypass.as_deref().map(wide), s.autoconfig_url.as_deref().map(wide));
        let ptr = |v: &mut Option<Vec<u16>>| v.as_mut().map(|w| w.as_mut_ptr()).unwrap_or(std::ptr::null_mut());
        let mut opts: [INTERNET_PER_CONN_OPTIONW; 4] = unsafe { std::mem::zeroed() };
        opts[0].dwOption = INTERNET_PER_CONN_FLAGS;
        opts[0].Value.dwValue = s.flags;
        opts[1].dwOption = INTERNET_PER_CONN_PROXY_SERVER;
        opts[1].Value.pszValue = ptr(&mut server);
        opts[2].dwOption = INTERNET_PER_CONN_PROXY_BYPASS;
        opts[2].Value.pszValue = ptr(&mut bypass);
        opts[3].dwOption = INTERNET_PER_CONN_AUTOCONFIG_URL;
        opts[3].Value.pszValue = ptr(&mut url);
        let list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            pszConnection: std::ptr::null_mut(),
            dwOptionCount: 4,
            dwOptionError: 0,
            pOptions: opts.as_mut_ptr(),
        };
        unsafe {
            if InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_PER_CONNECTION_OPTION, &list as *const _ as *const c_void, list.dwSize) == 0 {
                bail!("réglage du proxy impossible : {}", std::io::Error::last_os_error());
            }
            // Prévenir les applications que le réglage a changé.
            InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null(), 0);
            InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_REFRESH, std::ptr::null(), 0);
        }
        Ok(())
    }

    pub fn set_pac(url: &str) -> Result<()> {
        write(&Snapshot { flags: PROXY_TYPE_DIRECT | PROXY_TYPE_AUTO_PROXY_URL, server: None, bypass: None, autoconfig_url: Some(url.into()) })
    }

    pub fn is_pac(url: &str) -> Result<bool> {
        let s = read()?;
        Ok(s.flags & PROXY_TYPE_AUTO_PROXY_URL != 0 && s.autoconfig_url.as_deref() == Some(url))
    }
}
