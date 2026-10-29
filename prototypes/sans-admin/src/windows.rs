//! Windows : proxy de l'utilisateur par l'API WinINet (réglage par
//! utilisateur, sans droits d'administration) et démarrage de session
//! par la clé « Run » de l'utilisateur.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::Networking::WinInet::*;

/// Réglage de proxy de l'utilisateur, tel que WinINet le décrit.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub flags: u32,
    pub server: Option<String>,
    pub bypass: Option<String>,
    pub autoconfig_url: Option<String>,
}

impl Snapshot {
    pub fn describe(&self) -> String {
        let mut kinds = vec![];
        if self.flags & PROXY_TYPE_DIRECT != 0 { kinds.push("direct"); }
        if self.flags & PROXY_TYPE_PROXY != 0 { kinds.push("proxy"); }
        if self.flags & PROXY_TYPE_AUTO_PROXY_URL != 0 { kinds.push("script PAC"); }
        if self.flags & PROXY_TYPE_AUTO_DETECT != 0 { kinds.push("détection automatique"); }
        format!(
            "{} · serveur {:?} · exceptions {:?} · script {:?}",
            kinds.join(" + "),
            self.server.as_deref().unwrap_or("—"),
            self.bypass.as_deref().unwrap_or("—"),
            self.autoconfig_url.as_deref().unwrap_or("—")
        )
    }
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
    let mut opts: [INTERNET_PER_CONN_OPTIONW; 4] = Default::default();
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

fn write(s: &Snapshot) -> Result<()> {
    let (mut server, mut bypass, mut url) = (
        s.server.as_deref().map(wide),
        s.bypass.as_deref().map(wide),
        s.autoconfig_url.as_deref().map(wide),
    );
    let ptr = |v: &mut Option<Vec<u16>>| v.as_mut().map(|w| w.as_mut_ptr()).unwrap_or(std::ptr::null_mut());
    let mut opts: [INTERNET_PER_CONN_OPTIONW; 4] = Default::default();
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

pub fn ours(port: u16, pac: bool) -> Snapshot {
    if pac {
        Snapshot {
            flags: PROXY_TYPE_DIRECT | PROXY_TYPE_AUTO_PROXY_URL,
            server: None,
            bypass: None,
            autoconfig_url: Some(format!("http://127.0.0.1:{port}/proxy.pac")),
        }
    } else {
        Snapshot {
            flags: PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY,
            server: Some(format!("127.0.0.1:{port}")),
            bypass: Some("<local>".into()),
            autoconfig_url: None,
        }
    }
}

pub fn apply_local(port: u16, pac: bool) -> Result<()> {
    write(&ours(port, pac))
}

pub fn restore(s: &Snapshot) -> Result<()> {
    write(s)
}

/// Le réglage actuel est-il encore celui que l'agent a posé ?
pub fn is_ours(port: u16, pac: bool) -> bool {
    let o = ours(port, pac);
    read()
        .map(|s| if pac { s.flags & PROXY_TYPE_AUTO_PROXY_URL != 0 && s.autoconfig_url == o.autoconfig_url } else { s.flags & PROXY_TYPE_PROXY != 0 && s.server == o.server })
        .unwrap_or(false)
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("SurveoProto")
}

pub fn installed_exe() -> PathBuf {
    data_dir().join("sans-admin.exe")
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn register_autostart(exe: &std::path::Path) -> Result<String> {
    let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    key.set_value("SurveoProto", &format!("\"{}\" demarrage", exe.display()))?;
    Ok(format!(r"HKEY_CURRENT_USER\{RUN_KEY}\SurveoProto"))
}

pub fn unregister_autostart() -> Result<()> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE)?;
    let _ = key.delete_value("SurveoProto");
    Ok(())
}

pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}
