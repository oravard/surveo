//! Restrictions du poste pendant la séance (SPEC.md, chap. 2 ; IMPL.md,
//! 2.2).
//!
//! La console envoie l'état complet des restrictions voulues
//! ([`Restrictions`]) ; le [`Restrictor`] applique la différence avec ce
//! qui est en place. Il est autonome : il garde les restrictions pendant
//! une coupure (RESTR-2), lève le mode examen à son heure (EXAM-3), et
//! tout lève quand le poste quitte la séance (RESTR-1).
//!
//! Toute restriction est visible de l'élève (MAIT-1) : l'indicateur de
//! séance les énonce en mots simples (AGENT-5).
//!
//! Avant de modifier un réglage du poste, l'agent écrit la valeur
//! d'origine dans le journal de remise en état (`remise-en-etat.json`) ;
//! il l'efface après l'avoir rétablie. Un journal resté là au démarrage
//! vient d'une séance mal terminée : [`restore_leftovers`] rétablit
//! l'origine avant toute connexion (RESTR-3, DEMAR-2). L'agent ne rétablit
//! un réglage que s'il est encore le sien (RESTR-4).

use crate::core::Ui;
use crate::webfilter::{OnBlocked, WebFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use surveo_identity::{load_json, save_json};
use surveo_platform::apps::{self, Program};
use surveo_platform::proxy::UserProxy;
use surveo_protocol::{OpenApp, Restrictions, WebPolicy, WebState};
use tokio::time::Instant;

const JOURNAL_FILE: &str = "remise-en-etat.json";

/// Ce que l'agent a modifié sur le poste, et la valeur d'origine.
#[derive(Serialize, Deserialize, Default)]
struct Journal {
    #[serde(default)]
    proxy: Option<ProxyChange>,
}

/// Réglage du proxy de l'utilisateur remplacé par le script de l'agent.
#[derive(Serialize, Deserialize, Clone)]
struct ProxyChange {
    origin: String,
    pac_url: String,
}

/// Au démarrage, avant toute connexion : rétablit ce qu'une séance mal
/// terminée a laissé (RESTR-3, DEMAR-2).
pub fn restore_leftovers(dir: &Path, proxy: &dyn UserProxy) {
    let path = dir.join(JOURNAL_FILE);
    let Ok(j) = load_json::<Journal>(&path) else { return };
    log::info!("séance précédente mal terminée : retour à l'état d'origine");
    if let Some(p) = j.proxy {
        if !restore_proxy(proxy, &p) {
            return;
        }
    }
    let _ = std::fs::remove_file(path);
}

/// Rétablit le réglage d'origine s'il est encore celui de l'agent ; rend
/// `false` si le rétablissement a échoué, pour réessayer plus tard.
fn restore_proxy(proxy: &dyn UserProxy, p: &ProxyChange) -> bool {
    if !proxy.is_pac(&p.pac_url) {
        log::info!("réglage du proxy modifié par ailleurs : laissé tel quel (RESTR-4)");
        return true;
    }
    match proxy.restore(&p.origin) {
        Ok(()) => {
            log::info!("réglage du proxy d'origine rétabli");
            true
        }
        Err(e) => {
            log::warn!("réglage du proxy d'origine impossible à rétablir : {e:#}");
            false
        }
    }
}

/// Prévient l'élève qu'un site est bloqué (WEB-2), sans le harceler : un
/// même site au plus une fois par minute, une notification au plus toutes
/// les quatre secondes.
fn blocked_notifier(ui: Arc<dyn Ui>, teacher: String) -> OnBlocked {
    let seen = Mutex::new((HashMap::<String, std::time::Instant>::new(), None::<std::time::Instant>));
    Arc::new(move |host: &str| {
        let now = std::time::Instant::now();
        let mut g = seen.lock().unwrap();
        if g.1.is_some_and(|t| now - t < Duration::from_secs(4)) || g.0.get(host).is_some_and(|t| now - *t < Duration::from_secs(60)) {
            return;
        }
        g.0.insert(host.into(), now);
        g.1 = Some(now);
        drop(g);
        ui.notify(&format!("{host} n'est pas accessible pendant la séance de {teacher}."));
    })
}

/// Ce que l'indicateur de séance dit des restrictions en cours (MAIT-1).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct RestrictionsView {
    /// Fin du mode examen, sur l'horloge du poste (EXAM-1).
    pub exam_ends_at_ms: Option<u64>,
    /// Restrictions en cours, en mots simples.
    pub items: Vec<String>,
}

pub struct Restrictor {
    ui: Arc<dyn Ui>,
    dir: PathBuf,
    proxy: Arc<dyn UserProxy>,
    /// Enseignant de la séance, nommé dans les notifications (WEB-2, APP-1).
    teacher: String,
    wanted: Restrictions,
    exam_deadline: Option<Instant>,
    /// Message de l'écran en pause affiché.
    paused: Option<String>,
    web: Option<WebFilter>,
    proxy_change: Option<ProxyChange>,
    web_state: WebState,
    /// Processus en cours de fermeture, pour ne pas les fermer deux fois.
    closing: HashMap<u32, Instant>,
    /// Dernière notification de fermeture de chaque programme.
    notified: HashMap<String, Instant>,
    view: RestrictionsView,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Restrictor {
    pub fn new(ui: Arc<dyn Ui>, dir: PathBuf, proxy: Arc<dyn UserProxy>) -> Self {
        Self {
            ui,
            dir,
            proxy,
            teacher: String::new(),
            wanted: Restrictions::default(),
            exam_deadline: None,
            paused: None,
            web: None,
            proxy_change: None,
            web_state: WebState::Off,
            closing: HashMap::new(),
            notified: HashMap::new(),
            view: RestrictionsView::default(),
        }
    }

    /// Le poste entre dans une séance.
    pub fn begin(&mut self, teacher: &str) {
        self.teacher = teacher.into();
    }

    /// Applique les restrictions voulues par la console ; rend ce qui n'a
    /// pas pu l'être, pour l'acquittement (RETOUR-1).
    pub fn apply(&mut self, r: Restrictions) -> Vec<String> {
        self.exam_deadline = r.exam.as_ref().map(|e| Instant::now() + Duration::from_millis(e.remaining_ms));
        let exam_end = r.exam.as_ref().map(|e| now_ms() + e.remaining_ms);
        let banned_changed = r.banned_apps != self.wanted.banned_apps;
        self.wanted = r;
        let errors = self.sync();
        // Les applications interdites déjà ouvertes sont fermées aussitôt.
        if banned_changed {
            self.notified.clear();
            self.close_banned();
        }
        self.show(exam_end);
        errors
    }

    /// Lève tout : fin de la séance, ou démarrage (RESTR-1, RESTR-3).
    pub fn clear(&mut self) {
        self.wanted = Restrictions::default();
        self.exam_deadline = None;
        for e in self.sync() {
            log::warn!("remise en état incomplète : {e}");
        }
        self.show(None);
    }

    /// Vérifications périodiques : fin du mode examen, applications
    /// interdites, réglage du proxy. Rend `true` si l'état annoncé à la
    /// console a changé.
    pub fn tick(&mut self) -> bool {
        self.close_banned();
        let before = self.web_state.clone();
        if self.exam_deadline.is_some_and(|d| Instant::now() >= d) {
            log::info!("fin du mode examen");
            self.wanted.exam = None;
            self.exam_deadline = None;
            for e in self.sync() {
                log::warn!("fin du mode examen : {e}");
            }
            self.show(None);
        }
        // L'élève a changé le réglage du proxy : il est sorti du filtrage,
        // la console le saura (MODE-3).
        if let Some(p) = &self.proxy_change {
            let ours = self.proxy.is_pac(&p.pac_url);
            match (&self.web_state, ours) {
                (WebState::On, false) => {
                    log::warn!("sortie de restriction : le réglage du proxy a été modifié");
                    self.web_state = WebState::Escaped;
                }
                (WebState::Escaped, true) => self.web_state = WebState::On,
                _ => {}
            }
        }
        self.web_state != before
    }

    pub fn web_state(&self) -> WebState {
        self.web_state.clone()
    }

    /// Applications ouvertes, pour l'enseignant (APP-1).
    pub fn list_apps(&self) -> Vec<OpenApp> {
        apps::scan().into_iter().filter(|p| p.listed).map(|p| OpenApp { id: p.id, name: p.name }).collect()
    }

    /// Ferme une application à la demande de l'enseignant ; l'élève en
    /// est prévenu (APP-1).
    pub fn close_app(&mut self, id: &str) -> Result<(), String> {
        let p = apps::scan().into_iter().find(|p| p.id == id).ok_or("cette application n'est plus ouverte")?;
        log::info!("fermeture de {} demandée par l'enseignant", p.name);
        let name = p.name.clone();
        self.close(&p, &name);
        Ok(())
    }

    /// Ferme les applications interdites ouvertes (APP-2). Un programme du
    /// bureau n'est jamais fermé (APP-3).
    fn close_banned(&mut self) {
        if self.wanted.banned_apps.is_empty() {
            return;
        }
        let now = Instant::now();
        self.closing.retain(|_, t| now.duration_since(*t) < Duration::from_secs(8));
        let terms = self.wanted.banned_apps.clone();
        for p in apps::scan() {
            let Some(term) = terms.iter().find(|t| p.matches(t)) else { continue };
            if p.pids.iter().all(|pid| self.closing.contains_key(pid)) {
                continue;
            }
            // Un programme sans lanceur porte le nom donné par l'enseignant.
            let name = if p.listed { p.name.clone() } else { term.clone() };
            log::info!("application interdite fermée : {name}");
            self.close(&p, &name);
        }
    }

    fn close(&mut self, p: &Program, name: &str) {
        let now = Instant::now();
        apps::close(&p.pids);
        for pid in &p.pids {
            self.closing.insert(*pid, now);
        }
        // Une application relancée aussitôt n'est annoncée qu'une fois.
        if self.notified.get(&p.id).is_none_or(|t| now.duration_since(*t) > Duration::from_secs(30)) {
            self.notified.insert(p.id.clone(), now);
            self.ui.notify(&format!("{name} a été fermée par {}.", self.teacher));
        }
    }

    /// Filtrage web en vigueur : celui de l'examen, sinon celui du poste
    /// (EXAM-2).
    fn effective_web(&self) -> Option<WebPolicy> {
        match &self.wanted.exam {
            Some(e) => Some(WebPolicy::Allow { sites: e.sites.clone() }),
            None => self.wanted.web.clone(),
        }
    }

    /// Met le poste dans l'état voulu.
    fn sync(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.paused != self.wanted.pause {
            match self.ui.pause(self.wanted.pause.as_deref()) {
                Ok(()) => self.paused = self.wanted.pause.clone(),
                Err(e) => errors.push(format!("écran en pause impossible : {e}")),
            }
        }
        match (self.effective_web(), &self.web) {
            (None, None) => {}
            (None, Some(_)) => self.stop_web(),
            (Some(p), Some(f)) => {
                f.set_policy(p);
                // Réappliquer après une sortie de restriction rétablit le
                // réglage de l'agent (MODE-3).
                if self.web_state == WebState::Escaped {
                    let url = f.pac_url();
                    match self.proxy.set_pac(&url) {
                        Ok(()) => self.web_state = WebState::On,
                        Err(e) => errors.push(format!("filtrage web impossible à rétablir : {e:#}")),
                    }
                }
            }
            (Some(p), None) => {
                if let Err(e) = self.start_web(p) {
                    log::warn!("filtrage web impossible : {e:#}");
                    self.web_state = WebState::Unavailable { reason: format!("{e:#}") };
                    errors.push(format!("filtrage web impossible : {e:#}"));
                }
            }
        }
        errors
    }

    /// Pose le filtrage : proxy local, journal, puis réglage de
    /// l'utilisateur (WEB-1, MODE-5).
    fn start_web(&mut self, p: WebPolicy) -> anyhow::Result<()> {
        let origin = self.proxy.snapshot()?;
        let filter = WebFilter::start(p, &self.teacher, blocked_notifier(self.ui.clone(), self.teacher.clone()))?;
        let change = ProxyChange { origin, pac_url: filter.pac_url() };
        save_json(&self.dir.join(JOURNAL_FILE), &Journal { proxy: Some(change.clone()) }, false)?;
        if let Err(e) = self.proxy.set_pac(&change.pac_url) {
            let _ = std::fs::remove_file(self.dir.join(JOURNAL_FILE));
            return Err(e);
        }
        log::info!("filtrage web posé (proxy local sur le port {})", filter.port());
        self.web = Some(filter);
        self.proxy_change = Some(change);
        self.web_state = WebState::On;
        Ok(())
    }

    /// Lève le filtrage et rétablit le réglage d'origine (WEB-3).
    fn stop_web(&mut self) {
        if let Some(p) = self.proxy_change.take() {
            if restore_proxy(self.proxy.as_ref(), &p) {
                let _ = std::fs::remove_file(self.dir.join(JOURNAL_FILE));
            }
        }
        self.web = None;
        self.web_state = WebState::Off;
    }

    /// Met à jour l'indicateur de séance (MAIT-1).
    fn show(&mut self, exam_end: Option<u64>) {
        let mut items = Vec::new();
        match (&self.wanted.exam, self.effective_web()) {
            (Some(_), _) => items.push("Web : seuls les sites de l'examen sont ouverts".to_string()),
            (None, Some(WebPolicy::Allow { .. })) => items.push("Web : seuls quelques sites sont ouverts".into()),
            (None, Some(WebPolicy::Block { .. })) => items.push("Web : certains sites sont bloqués".into()),
            (None, Some(WebPolicy::All)) => items.push("Web : bloqué".into()),
            (None, None) => {}
        }
        if !self.wanted.banned_apps.is_empty() {
            items.push(format!("Applications interdites : {}", self.wanted.banned_apps.join(", ")));
        }
        let exam_ends_at_ms = match (self.wanted.exam.is_some(), exam_end) {
            (true, Some(t)) => Some(t),
            (true, None) => self.view.exam_ends_at_ms,
            (false, _) => None,
        };
        // Même fin d'examen à quelques secondes près : l'affichage ne bouge pas.
        let exam_ends_at_ms = match (exam_ends_at_ms, self.view.exam_ends_at_ms) {
            (Some(a), Some(b)) if a.abs_diff(b) < 30_000 => Some(b),
            (a, _) => a,
        };
        let view = RestrictionsView { exam_ends_at_ms, items };
        if view != self.view {
            self.ui.restrictions(&view);
            self.view = view;
        }
    }

    pub fn teacher(&self) -> &str {
        &self.teacher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SessionView;
    use surveo_platform::proxy::MemoryProxy;
    use surveo_protocol::Exam;

    #[derive(Default)]
    struct TestUi {
        view: Mutex<RestrictionsView>,
        pause: Mutex<Option<String>>,
    }

    impl Ui for TestUi {
        fn session(&self, _: Option<&SessionView>) {}
        fn identify(&self, _: &str, _: u32) {}
        fn restrictions(&self, r: &RestrictionsView) {
            *self.view.lock().unwrap() = r.clone();
        }
        fn pause(&self, m: Option<&str>) -> Result<(), String> {
            *self.pause.lock().unwrap() = m.map(Into::into);
            Ok(())
        }
        fn notify(&self, _: &str) {}
    }

    const ORIGIN: &str = "réglage du service informatique";

    fn restrictor() -> (Restrictor, Arc<TestUi>, Arc<MemoryProxy>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("surveo-restrict-{}", surveo_identity::random_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ui = Arc::new(TestUi::default());
        let proxy = Arc::new(MemoryProxy::default());
        *proxy.value.lock().unwrap() = ORIGIN.into();
        let mut r = Restrictor::new(ui.clone(), dir.clone(), proxy.clone());
        r.begin("M. Durand");
        (r, ui, proxy, dir)
    }

    fn web(p: WebPolicy) -> Restrictions {
        Restrictions { web: Some(p), ..Default::default() }
    }

    #[tokio::test]
    async fn web_filter_is_journaled_and_origin_restored() {
        let (mut r, ui, proxy, dir) = restrictor();
        assert!(r.apply(web(WebPolicy::All)).is_empty());
        assert!(proxy.value.lock().unwrap().starts_with("pac http://127.0.0.1:"));
        assert!(dir.join(JOURNAL_FILE).exists(), "journal écrit avant de toucher au réglage");
        assert_eq!(r.web_state(), WebState::On);
        assert_eq!(ui.view.lock().unwrap().items, ["Web : bloqué"]);

        // Changement de politique : même proxy, même réglage.
        let pac = proxy.value.lock().unwrap().clone();
        r.apply(web(WebPolicy::Allow { sites: vec!["wikipedia.org".into()] }));
        assert_eq!(*proxy.value.lock().unwrap(), pac);

        r.clear();
        assert_eq!(*proxy.value.lock().unwrap(), ORIGIN);
        assert!(!dir.join(JOURNAL_FILE).exists());
        assert_eq!(r.web_state(), WebState::Off);
        assert!(ui.view.lock().unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn leaving_the_filter_is_detected_and_reapplying_restores_it() {
        let (mut r, _, proxy, _) = restrictor();
        r.apply(web(WebPolicy::All));
        let pac = proxy.value.lock().unwrap().clone();
        assert!(!r.tick());
        *proxy.value.lock().unwrap() = "direct".into();
        assert!(r.tick());
        assert_eq!(r.web_state(), WebState::Escaped);
        r.apply(web(WebPolicy::All));
        assert_eq!((r.web_state(), proxy.value.lock().unwrap().clone()), (WebState::On, pac));

        // Réglage changé par ailleurs puis séance finie : rien n'est
        // écrasé (RESTR-4).
        *proxy.value.lock().unwrap() = "nouveau réglage".into();
        r.clear();
        assert_eq!(*proxy.value.lock().unwrap(), "nouveau réglage");
    }

    #[tokio::test]
    async fn startup_restores_what_a_crashed_session_left() {
        let (mut r, _, proxy, dir) = restrictor();
        r.apply(web(WebPolicy::All));
        std::mem::forget(r); // arrêt brutal : rien n'est rétabli
        assert!(dir.join(JOURNAL_FILE).exists());
        restore_leftovers(&dir, proxy.as_ref());
        assert_eq!(*proxy.value.lock().unwrap(), ORIGIN);
        assert!(!dir.join(JOURNAL_FILE).exists());
        restore_leftovers(&dir, proxy.as_ref());
        assert_eq!(*proxy.value.lock().unwrap(), ORIGIN);
    }

    #[test]
    fn blocked_sites_are_announced_sparingly() {
        #[derive(Default)]
        struct Notices(Mutex<Vec<String>>);
        impl Ui for Notices {
            fn session(&self, _: Option<&SessionView>) {}
            fn identify(&self, _: &str, _: u32) {}
            fn restrictions(&self, _: &RestrictionsView) {}
            fn pause(&self, _: Option<&str>) -> Result<(), String> {
                Ok(())
            }
            fn notify(&self, t: &str) {
                self.0.lock().unwrap().push(t.into());
            }
        }
        let ui = Arc::new(Notices::default());
        let notify = blocked_notifier(ui.clone(), "M. Durand".into());
        notify("jeux.example");
        notify("autre.example");
        notify("jeux.example");
        assert_eq!(*ui.0.lock().unwrap(), ["jeux.example n'est pas accessible pendant la séance de M. Durand."]);
    }

    #[tokio::test]
    async fn exam_replaces_the_filter_until_its_end() {
        let (mut r, ui, _, _) = restrictor();
        let mut rs = web(WebPolicy::Block { sites: vec!["jeux.fr".into()] });
        rs.exam = Some(Exam { remaining_ms: 0, sites: vec![] });
        rs.pause = Some("Regardez le tableau.".into());
        r.apply(rs);
        assert_eq!(r.effective_web(), Some(WebPolicy::Allow { sites: vec![] }));
        assert!(ui.view.lock().unwrap().exam_ends_at_ms.is_some());
        assert_eq!(ui.pause.lock().unwrap().as_deref(), Some("Regardez le tableau."));
        r.tick();
        assert_eq!(r.effective_web(), Some(WebPolicy::Block { sites: vec!["jeux.fr".into()] }));
        assert_eq!(*ui.view.lock().unwrap(), RestrictionsView { exam_ends_at_ms: None, items: vec!["Web : certains sites sont bloqués".into()] });
        r.clear();
        assert_eq!(*ui.pause.lock().unwrap(), None);
    }
}
