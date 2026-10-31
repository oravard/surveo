//! Applications de la session de l'utilisateur (SPEC.md, APP-1 à APP-3 ;
//! IMPL.md, 2.2), sans droits d'administration : l'agent ne voit et ne
//! ferme que les programmes de la session où il tourne (MODE-1).
//!
//! - Linux : les processus de l'utilisateur, rapprochés des lanceurs
//!   d'applications (fichiers `.desktop`) pour leur nom ; fonctionne sous
//!   Wayland comme sous X11.
//! - Windows : les processus de la session, et leurs fenêtres pour savoir
//!   lesquels sont des applications ; le nom vient de la description de
//!   l'exécutable.
//!
//! Un programme du bureau lui-même (barre des tâches, gestionnaire de
//! fenêtres…) n'est jamais listé ni fermé (APP-3).

use std::collections::HashMap;

/// Un programme en cours dans la session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// Le désigne pour le fermer.
    pub id: String,
    /// Nom lisible (« Firefox », « Minecraft Launcher »).
    pub name: String,
    /// C'est une application que l'enseignant voit (lanceur ou fenêtre),
    /// et non un programme d'arrière-plan.
    pub listed: bool,
    pub pids: Vec<u32>,
    /// Noms d'exécutable, en minuscules et sans « .exe ».
    exes: Vec<String>,
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

fn exe_key(s: &str) -> String {
    let s = s.trim().to_lowercase();
    s.strip_suffix(".exe").map(Into::into).unwrap_or(s)
}

impl Program {
    /// Le programme est-il celui que l'enseignant a nommé (APP-2) ? Le nom
    /// donné doit être celui de l'exécutable, ou des mots entiers du nom
    /// de l'application : « minecraft » désigne « Minecraft Launcher »,
    /// pas « Minecraftien ».
    pub fn matches(&self, term: &str) -> bool {
        let key = exe_key(term);
        if key.is_empty() {
            return false;
        }
        if self.exes.iter().any(|e| *e == key) {
            return true;
        }
        let (t, n) = (words(term), words(&self.name));
        !t.is_empty() && n.windows(t.len()).any(|w| w == t.as_slice())
    }
}

/// Programmes du bureau, jamais listés ni fermés (APP-3).
const DESKTOP: &[&str] = &[
    // Linux
    "gnome-shell", "gnome-session-binary", "plasmashell", "ksmserver", "kded6", "kded5", "krunner", "kwin", "kwin_wayland",
    "kwin_x11", "mutter", "xwayland", "xorg", "systemd", "dbus-daemon", "dbus-broker", "pipewire", "pipewire-pulse",
    "wireplumber", "pulseaudio", "cosmic-comp", "cosmic-panel", "cosmic-session", "cosmic-launcher", "cosmic-bg",
    "cosmic-osd", "cosmic-greeter", "cosmic-workspaces", "cosmic-app-library", "cosmic-notifications", "cosmic-idle",
    "cosmic-settings-daemon", "xfce4-panel", "xfwm4", "xfdesktop", "xfce4-session", "lxpanel", "lxsession", "openbox",
    "cinnamon", "muffin", "marco", "mate-panel", "mate-session", "budgie-panel", "budgie-wm", "sddm", "gdm", "lightdm",
    "surveo-agent", "surveo-console",
    // Windows
    "explorer", "dwm", "csrss", "winlogon", "sihost", "taskhostw", "ctfmon", "runtimebroker", "applicationframehost",
    "shellexperiencehost", "startmenuexperiencehost", "searchhost", "searchapp", "textinputhost", "lockapp",
    "systemsettings", "securityhealthsystray", "smartscreen", "fontdrvhost", "dllhost", "conhost", "svchost",
];
const DESKTOP_PREFIXES: &[&str] = &["gsd-", "xdg-", "at-spi", "ibus", "fcitx", "polkit", "cosmic-applet", "gnome-shell-", "kwin_", "webkit"];

fn is_desktop(exe: &str) -> bool {
    let e = exe_key(exe);
    DESKTOP.contains(&e.as_str()) || DESKTOP_PREFIXES.iter().any(|p| e.starts_with(p))
}

/// Un processus vu par le système.
#[derive(Clone, Debug)]
struct Proc {
    pid: u32,
    ppid: u32,
    /// Noms possibles du programme (nom du processus, exécutable…).
    exes: Vec<String>,
}

/// Regroupe les processus en programmes. `app_of` rend, pour un processus,
/// l'application connue à laquelle il appartient : (identifiant, nom).
/// L'agent `me` et sa descendance (ses vues web) sont écartés.
fn group(procs: Vec<Proc>, me: u32, app_of: impl Fn(&Proc) -> Option<(String, String)>) -> Vec<Program> {
    let parent: HashMap<u32, u32> = procs.iter().map(|p| (p.pid, p.ppid)).collect();
    let mine = |mut pid: u32| {
        for _ in 0..64 {
            if pid == me {
                return true;
            }
            match parent.get(&pid) {
                Some(&pp) if pp != pid && pp != 0 => pid = pp,
                _ => return false,
            }
        }
        false
    };
    let mut out: Vec<Program> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for p in procs {
        if mine(p.pid) || p.exes.is_empty() || p.exes.iter().any(|e| is_desktop(e)) {
            continue;
        }
        let (id, name, listed) = match app_of(&p) {
            Some((id, name)) => (id, name, true),
            None => (format!("exe:{}", p.exes[0]), p.exes[0].clone(), false),
        };
        let i = *index.entry(id.clone()).or_insert_with(|| {
            out.push(Program { id, name, listed, pids: vec![], exes: vec![] });
            out.len() - 1
        });
        let prog = &mut out[i];
        prog.pids.push(p.pid);
        for e in p.exes {
            if !prog.exes.contains(&e) {
                prog.exes.push(e);
            }
        }
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Programmes en cours dans la session de l'utilisateur.
pub fn scan() -> Vec<Program> {
    imp::scan()
}

/// Ferme des processus : d'abord poliment, puis de force s'ils sont encore
/// là quelques secondes plus tard.
pub fn close(pids: &[u32]) {
    imp::close(pids)
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Une application installée, d'après son lanceur.
    #[derive(Clone, Debug)]
    pub(super) struct Entry {
        pub id: String,
        pub name: String,
        /// Noms d'exécutable qui la désignent.
        pub programs: Vec<String>,
        /// Dossier propre de l'exécutable (`/opt/google/chrome`), qui
        /// désigne aussi ses processus.
        pub dir: Option<PathBuf>,
    }

    fn basename(s: &str) -> String {
        let s = s.trim_end_matches(" (deleted)");
        s.rsplit('/').next().unwrap_or(s).to_lowercase()
    }

    /// Découpe une ligne `Exec=` en mots, guillemets compris.
    fn split_exec(s: &str) -> Vec<String> {
        let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
        for c in s.chars() {
            match c {
                '"' => quoted = !quoted,
                c if c.is_whitespace() && !quoted => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }

    fn which(program: &str) -> Option<PathBuf> {
        if program.contains('/') {
            return Some(PathBuf::from(program));
        }
        std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(program)).find(|f| f.is_file()))
    }

    const COMMON_DIRS: &[&str] = &["/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin", "/usr/games", "/app/bin"];

    /// Lit un lanceur ; rien s'il ne désigne pas une application visible.
    pub(super) fn parse_entry(id: &str, text: &str) -> Option<Entry> {
        let mut in_main = false;
        let (mut name, mut name_fr, mut exec, mut wmclass) = (None, None, None, None);
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_main = line == "[Desktop Entry]";
                continue;
            }
            if !in_main {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "Type" if v.trim() != "Application" => return None,
                "NoDisplay" | "Hidden" | "Terminal" if v.trim() == "true" => return None,
                "Name" => name = Some(v.trim().to_string()),
                "Name[fr]" => name_fr = Some(v.trim().to_string()),
                "Exec" => exec = Some(v.trim().to_string()),
                "StartupWMClass" => wmclass = Some(v.trim().to_lowercase()),
                _ => {}
            }
        }
        let name = name_fr.or(name)?;
        let words: Vec<String> = split_exec(&exec?).into_iter().filter(|w| !w.contains('=') || w.starts_with("--")).collect();
        let program = words.iter().find(|w| *w != "env")?.clone();
        let mut programs = Vec::new();
        let mut dir = None;
        if basename(&program) == "flatpak" {
            // « flatpak run --command=firefox org.mozilla.firefox »
            match words.iter().find_map(|w| w.strip_prefix("--command=")) {
                Some(c) => programs.push(basename(c)),
                None => {
                    if let Some(app) = words.iter().skip(1).filter(|w| !w.starts_with('-') && !w.contains('%') && *w != "run").last() {
                        programs.push(app.rsplit('.').next().unwrap_or(app).to_lowercase());
                    }
                }
            }
        } else {
            programs.push(basename(&program));
            dir = which(&program)
                .and_then(|p| p.canonicalize().ok())
                .and_then(|p| p.parent().map(Path::to_path_buf))
                .filter(|d| !COMMON_DIRS.iter().any(|c| d == Path::new(c)));
        }
        programs.extend(wmclass);
        programs.retain(|p| !p.is_empty() && !is_desktop(p));
        if programs.is_empty() {
            return None;
        }
        Some(Entry { id: id.into(), name, programs, dir })
    }

    fn entry_dirs() -> Vec<PathBuf> {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
        let data_dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let mut dirs = vec![data_home.join("applications")];
        dirs.extend(data_dirs.split(':').map(|d| PathBuf::from(d).join("applications")));
        dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
        dirs.push(data_home.join("flatpak/exports/share/applications"));
        dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));
        dirs
    }

    /// Lanceurs installés ; le premier trouvé l'emporte, comme pour le
    /// bureau.
    fn catalog() -> Vec<Entry> {
        static CACHE: Mutex<Option<(Instant, Vec<Entry>)>> = Mutex::new(None);
        let mut c = CACHE.lock().unwrap();
        if let Some((at, e)) = &*c {
            if at.elapsed() < Duration::from_secs(60) {
                return e.clone();
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for d in entry_dirs() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for f in rd.flatten() {
                let path = f.path();
                let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".desktop")) else { continue };
                if !seen.insert(id.to_string()) {
                    continue;
                }
                if let Some(e) = std::fs::read_to_string(&path).ok().and_then(|t| parse_entry(id, &t)) {
                    out.push(e);
                }
            }
        }
        *c = Some((Instant::now(), out.clone()));
        out
    }

    fn uid_of(status: &str) -> Option<u32> {
        status.lines().find_map(|l| l.strip_prefix("Uid:")).and_then(|v| v.split_whitespace().next()?.parse().ok())
    }

    fn ppid_of(status: &str) -> u32 {
        status.lines().find_map(|l| l.strip_prefix("PPid:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
    }

    /// Processus de l'utilisateur, avec le dossier de leur exécutable.
    fn processes() -> Vec<(Proc, Option<PathBuf>)> {
        let me = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| uid_of(&s));
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir("/proc") else { return out };
        for d in rd.flatten() {
            let Some(pid) = d.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
            let base = d.path();
            let Ok(status) = std::fs::read_to_string(base.join("status")) else { continue };
            if uid_of(&status) != me {
                continue;
            }
            let exe = std::fs::read_link(base.join("exe")).ok();
            let mut exes = Vec::new();
            if let Some(e) = &exe {
                exes.push(basename(&e.to_string_lossy()));
            }
            if let Ok(cmd) = std::fs::read(base.join("cmdline")) {
                if let Some(a0) = cmd.split(|b| *b == 0).next().filter(|a| !a.is_empty()) {
                    // Certains programmes réécrivent leur ligne de commande
                    // en un seul mot : « /opt/x/chrome --type=renderer ».
                    let a0 = String::from_utf8_lossy(a0);
                    exes.push(basename(a0.split(" --").next().unwrap_or(&a0)));
                }
            }
            if let Ok(comm) = std::fs::read_to_string(base.join("comm")) {
                exes.push(comm.trim().to_lowercase());
            }
            exes.dedup();
            exes.retain(|e| !e.is_empty());
            // Un processus sans exécutable lisible est un fil du noyau.
            if exe.is_none() && exes.is_empty() {
                continue;
            }
            let dir = exe.and_then(|e| e.parent().map(Path::to_path_buf));
            out.push((Proc { pid, ppid: ppid_of(&status), exes }, dir));
        }
        out
    }

    pub(super) fn app_for<'a>(entries: &'a [Entry], p: &Proc, dir: Option<&Path>) -> Option<&'a Entry> {
        // Le nom du processus est tronqué à 15 caractères par le noyau.
        let same = |prog: &str, e: &str| prog == e || (e.len() == 15 && prog.starts_with(e));
        entries
            .iter()
            .find(|en| en.programs.iter().any(|prog| p.exes.iter().any(|e| same(prog, e))))
            .or_else(|| dir.and_then(|d| entries.iter().find(|en| en.dir.as_deref() == Some(d))))
    }

    pub fn scan() -> Vec<Program> {
        let entries = catalog();
        let procs = processes();
        let dirs: HashMap<u32, Option<PathBuf>> = procs.iter().map(|(p, d)| (p.pid, d.clone())).collect();
        group(procs.into_iter().map(|(p, _)| p).collect(), std::process::id(), |p| {
            app_for(&entries, p, dirs.get(&p.pid).and_then(|d| d.as_deref())).map(|e| (e.id.clone(), e.name.clone()))
        })
    }

    fn start_time(pid: u32) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Le nom entre parenthèses peut contenir des espaces.
        stat.rsplit_once(')').and_then(|(_, r)| r.split_whitespace().nth(19).map(Into::into))
    }

    pub fn close(pids: &[u32]) {
        let started: Vec<(u32, Option<String>)> = pids.iter().map(|&p| (p, start_time(p))).collect();
        for &(pid, _) in &started {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(4));
            for (pid, t) in started {
                // Même processus (et non un nouveau qui aurait repris son numéro).
                if t.is_some() && start_time(pid) == t {
                    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
                }
            }
        });
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn launchers_are_read() {
            let e = parse_entry("org.mozilla.firefox", "[Desktop Entry]\nType=Application\nName=Firefox\nName[fr]=Navigateur Firefox\nExec=flatpak run --branch=stable --command=firefox org.mozilla.firefox %u\n[Desktop Action new]\nName=Nouvelle fenêtre\n").unwrap();
            assert_eq!((e.name.as_str(), e.programs.as_slice()), ("Navigateur Firefox", &["firefox".to_string()][..]));
            let e = parse_entry("jeu", "[Desktop Entry]\nType=Application\nName=Minecraft Launcher\nExec=env GDK_BACKEND=x11 \"/opt/minecraft launcher/minecraft-launcher\" %U\nStartupWMClass=Minecraft\n").unwrap();
            assert_eq!(e.programs, ["minecraft-launcher", "minecraft"]);
            assert!(parse_entry("cache", "[Desktop Entry]\nType=Application\nName=x\nExec=x\nNoDisplay=true\n").is_none());
            assert!(parse_entry("shell", "[Desktop Entry]\nType=Application\nName=GNOME Shell\nExec=gnome-shell\n").is_none());
        }

        #[test]
        fn processes_are_grouped_by_application() {
            let entries = vec![Entry { id: "firefox".into(), name: "Firefox".into(), programs: vec!["firefox".into()], dir: None }];
            let procs = vec![
                Proc { pid: 10, ppid: 1, exes: vec!["firefox".into()] },
                Proc { pid: 11, ppid: 10, exes: vec!["firefox-bin".into(), "isolated web co".into()] },
                Proc { pid: 12, ppid: 1, exes: vec!["gnome-shell".into()] },
                Proc { pid: 13, ppid: 1, exes: vec!["javaw".into()] },
                Proc { pid: 14, ppid: 99, exes: vec!["webkitwebprocess".into()] },
                Proc { pid: 15, ppid: 14, exes: vec!["msedgewebview2".into()] },
            ];
            let got = group(procs, 99, |p| app_for(&entries, p, None).map(|e| (e.id.clone(), e.name.clone())));
            assert_eq!(got.iter().map(|p| (p.name.as_str(), p.listed, p.pids.clone())).collect::<Vec<_>>(), vec![
                ("Firefox", true, vec![10]),
                ("firefox-bin", false, vec![11]),
                ("javaw", false, vec![13]),
            ]);
        }

        #[test]
        fn a_running_program_is_found_and_closed() {
            // Une copie de `sleep` sous un nom propre au test.
            let dir = std::env::temp_dir().join(format!("surveo-apps-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let exe = dir.join("surveo-test-jeu");
            std::fs::copy("/bin/sleep", &exe).unwrap();
            // Lancé détaché, comme une application de l'élève : un enfant de
            // ce processus serait écarté, comme les vues web de l'agent.
            let out = std::process::Command::new("sh").arg("-c").arg(format!("'{}' 60 >/dev/null 2>&1 & echo $!", exe.display())).output().unwrap();
            let pid: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
            std::thread::sleep(Duration::from_millis(200));
            let found: Vec<Program> = scan().into_iter().filter(|p| p.matches("Surveo-Test-Jeu")).collect();
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].pids, [pid]);
            close(&found[0].pids);
            let gone = (0..50).any(|_| {
                std::thread::sleep(Duration::from_millis(100));
                start_time(pid).is_none()
            });
            assert!(gone, "processus encore là");
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_TERMINATE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GWL_EXSTYLE,
        GW_OWNER, WM_CLOSE, WS_EX_TOOLWINDOW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Fenêtres d'application : visibles, sans propriétaire, avec un titre.
    fn app_windows() -> Vec<(HWND, u32)> {
        unsafe extern "system" fn each(hwnd: HWND, l: LPARAM) -> BOOL {
            let out = &mut *(l as *mut Vec<(HWND, u32)>);
            if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() || GetWindowTextLengthW(hwnd) == 0 {
                return 1;
            }
            if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
                return 1;
            }
            let mut cloaked: u32 = 0;
            DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut _, 4);
            if cloaked != 0 {
                return 1;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            out.push((hwnd, pid));
            1
        }
        let mut out: Vec<(HWND, u32)> = Vec::new();
        unsafe { EnumWindows(Some(each), &mut out as *mut _ as LPARAM) };
        out
    }

    fn session_of(pid: u32) -> Option<u32> {
        let mut s = 0;
        (unsafe { ProcessIdToSessionId(pid, &mut s) } != 0).then_some(s)
    }

    fn image_path(pid: u32) -> Option<String> {
        unsafe {
            let h: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut n = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n);
            CloseHandle(h);
            (ok != 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
        }
    }

    /// Description de l'exécutable (« Firefox »), gardée en mémoire.
    fn description(path: &str) -> Option<String> {
        static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
        let mut c = CACHE.lock().unwrap();
        let c = c.get_or_insert_with(HashMap::new);
        if let Some(d) = c.get(path) {
            return d.clone();
        }
        let d = unsafe { read_description(path) };
        c.insert(path.into(), d.clone());
        d
    }

    unsafe fn read_description(path: &str) -> Option<String> {
        let p = wide(path);
        let size = GetFileVersionInfoSizeW(p.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(p.as_ptr(), 0, size, data.as_mut_ptr() as *mut _) == 0 {
            return None;
        }
        let mut tr: *mut u16 = std::ptr::null_mut();
        let mut len = 0u32;
        let key = wide("\\VarFileInfo\\Translation");
        if VerQueryValueW(data.as_ptr() as *const _, key.as_ptr(), &mut tr as *mut _ as *mut _, &mut len) == 0 || len < 4 {
            return None;
        }
        let (lang, cp) = (*tr, *tr.add(1));
        let key = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription"));
        let mut s: *mut u16 = std::ptr::null_mut();
        if VerQueryValueW(data.as_ptr() as *const _, key.as_ptr(), &mut s as *mut _ as *mut _, &mut len) == 0 || len == 0 {
            return None;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(s, len as usize - 1)).trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    fn processes() -> Vec<Proc> {
        let mine = session_of(unsafe { GetCurrentProcessId() });
        let mut out = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                let exe = exe_key(&String::from_utf16_lossy(&e.szExeFile[..n]));
                if mine.is_some() && session_of(e.th32ProcessID) == mine {
                    out.push(Proc { pid: e.th32ProcessID, ppid: e.th32ParentProcessID, exes: vec![exe] });
                }
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
        }
        out
    }

    pub fn scan() -> Vec<Program> {
        let windows: std::collections::HashSet<u32> = app_windows().into_iter().map(|(_, p)| p).collect();
        group(processes(), unsafe { GetCurrentProcessId() }, |p| {
            if !windows.contains(&p.pid) {
                return None;
            }
            let path = image_path(p.pid)?;
            let name = description(&path).unwrap_or_else(|| p.exes[0].clone());
            Some((path.to_lowercase(), name))
        })
    }

    pub fn close(pids: &[u32]) {
        let pids = pids.to_vec();
        for (hwnd, pid) in app_windows() {
            if pids.contains(&pid) {
                unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            }
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(4));
            for pid in pids {
                unsafe {
                    let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
                    if h.is_null() {
                        continue;
                    }
                    // Encore là : fermeture forcée.
                    if WaitForSingleObject(h, 0) != 0 {
                        TerminateProcess(h, 1);
                    }
                    CloseHandle(h);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(name: &str, exes: &[&str]) -> Program {
        Program { id: name.into(), name: name.into(), listed: true, pids: vec![1], exes: exes.iter().map(|e| e.to_string()).collect() }
    }

    #[test]
    fn programs_are_named_by_whole_words_or_executable() {
        let mc = prog("Minecraft Launcher", &["minecraftlauncher"]);
        assert!(mc.matches("minecraft") && mc.matches("Minecraft Launcher") && mc.matches("MinecraftLauncher.exe"));
        assert!(!mc.matches("craft") && !mc.matches("") && !prog("Minecraftien", &["x"]).matches("minecraft"));
        assert!(prog("Steam", &["steam"]).matches(" steam "));
        assert!(is_desktop("Explorer.EXE") && is_desktop("gsd-power") && !is_desktop("firefox"));
    }
}
