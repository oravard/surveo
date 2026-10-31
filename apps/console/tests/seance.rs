//! Séance minimale de bout en bout (IMPL.md, 1.2) : relais réel sur ce
//! poste, moteur de la console attaché au relais, et un agent réel qui
//! trouve le relais par la recherche locale et envoie des miniatures de
//! démonstration.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use surveo_agent::core::{self, AgentSetup, SessionView, Ui};
use surveo_agent::restrict::RestrictionsView;
use surveo_console::engine::Engine;
use surveo_console::store::Store;
use surveo_console::view::{ConsoleView, StationState};
use surveo_console::{now_ms, DEFAULT_DURATION_MS};
use surveo_identity::{new_member_id, random_id, Identity};
use surveo_platform::proxy::MemoryProxy;
use surveo_protocol::{decode_console_frame, ScreenState, ToConsole, WebPolicy, WebState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use surveo_relay_core::{Frame, Relay, RelayConfig};

/// Port de recherche propre au test, pour ne pas gêner une console en
/// fonctionnement sur ce poste.
const DISCOVERY_PORT: u16 = 47961;

#[derive(Default)]
struct RecordingUi {
    session: Mutex<Option<SessionView>>,
    identified: Mutex<Vec<String>>,
    restrictions: Mutex<RestrictionsView>,
    pause: Mutex<Option<String>>,
    notices: Mutex<Vec<String>>,
}

impl Ui for RecordingUi {
    fn session(&self, s: Option<&SessionView>) {
        *self.session.lock().unwrap() = s.cloned();
    }
    fn identify(&self, number: &str, _seconds: u32) {
        self.identified.lock().unwrap().push(number.into());
    }
    fn restrictions(&self, r: &RestrictionsView) {
        *self.restrictions.lock().unwrap() = r.clone();
    }
    fn pause(&self, message: Option<&str>) -> Result<(), String> {
        *self.pause.lock().unwrap() = message.map(Into::into);
        Ok(())
    }
    fn notify(&self, text: &str) {
        self.notices.lock().unwrap().push(text.into());
    }
}

/// Attend qu'une condition soit vraie, au plus 10 s.
async fn until(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..200 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("délai dépassé : {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn minimal_session_from_station_to_console() {
    let tmp = std::env::temp_dir().join(format!("surveo-e2e-{}", random_id()));

    // Console : établissement, salle, relais intégré.
    let mut engine = Engine::new(Store::new(tmp.join("console")), now_ms()).unwrap();
    engine.create_establishment("Lycée test", "Mme Martin").unwrap();
    let room = engine.create_room("B204", 6, 5).unwrap();
    let (identity, trust, console) = engine.relay_identity().unwrap();
    let relay = Relay::start(RelayConfig { identity, trust, port: 0, discovery: Some(DISCOVERY_PORT) }).await.unwrap();
    let local = relay.attach_console(&console);
    engine.attach(Box::new(local.sender.clone()));
    let engine = Arc::new(Mutex::new(engine));
    let thumbs = Arc::new(AtomicUsize::new(0));
    {
        let (engine, thumbs) = (engine.clone(), thumbs.clone());
        let mut frames = local.frames;
        tokio::spawn(async move {
            while let Some(f) = frames.recv().await {
                match f {
                    Frame::Text(t) => engine.lock().unwrap().handle(serde_json::from_str::<ToConsole>(&t).unwrap(), now_ms()),
                    Frame::Binary(b) => {
                        let (_, _, jpeg) = decode_console_frame(&b).unwrap();
                        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
                        thumbs.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }
    let view = |e: &Arc<Mutex<Engine>>| -> ConsoleView { e.lock().unwrap().view() };

    // Agent : paquet de la salle sans adresse de relais, il doit le trouver
    // par la recherche locale (DECOUV-1, niveau 3).
    let package = engine.lock().unwrap().package(&room).unwrap();
    assert!(package.relays.is_empty());
    let identity = Identity::enroll_agent(&package, &new_member_id("poste")).unwrap();
    let mut setup = AgentSetup::new(tmp.join("agent"), package, identity);
    setup.synthetic_screen = Some(1);
    setup.discovery_port = DISCOVERY_PORT;
    let proxy = Arc::new(MemoryProxy::default());
    *proxy.value.lock().unwrap() = "réglage d'origine".into();
    setup.proxy = proxy.clone();
    let ui = Arc::new(RecordingUi::default());
    tokio::spawn(core::run(setup, ui.clone()));

    // Le poste apparaît allumé, à placer (SALLE-1, SALLE-3).
    until("poste allumé", || view(&engine).room.unwrap().stations.first().is_some_and(|s| s.state == StationState::On)).await;
    let st = view(&engine).room.unwrap().stations[0].clone();
    assert_eq!((st.number.as_str(), st.pos), ("01", None));

    // Identifier (SALLE-4) : le poste affiche le numéro de sa tuile.
    engine.lock().unwrap().identify(&[], now_ms()).unwrap();
    until("numéro affiché", || ui.identified.lock().unwrap().as_slice() == ["01"]).await;

    // Ouverture : le poste rejoint la séance et affiche l'indicateur (REJ-1, REJ-4).
    let end = now_ms() + DEFAULT_DURATION_MS;
    engine.lock().unwrap().open_session(end, now_ms()).unwrap();
    until("poste dans la séance", || view(&engine).room.unwrap().stations[0].state == StationState::InSession).await;
    let shown = ui.session.lock().unwrap().clone().unwrap();
    assert_eq!(shown.teacher, "Mme Martin");
    assert!(shown.ends_at_ms.abs_diff(end) < 5_000);

    // Écrans affichés (AFF-4) : les miniatures arrivent à la console.
    engine.lock().unwrap().set_screens(true).unwrap();
    until("miniatures reçues", || thumbs.load(Ordering::Relaxed) >= 2).await;
    assert_eq!(view(&engine).room.unwrap().stations[0].screen, ScreenState::On);

    // Prolongation (SEANCE-3) : l'indicateur suit la nouvelle heure de fin.
    let later = end + 15 * 60_000;
    engine.lock().unwrap().set_end(later, now_ms()).unwrap();
    until("heure de fin prolongée", || ui.session.lock().unwrap().as_ref().is_some_and(|s| s.ends_at_ms.abs_diff(later) < 5_000)).await;

    // Écran en pause avec un message, puis repris (ECRAN-1 à ECRAN-3).
    let paused = |m: Option<&str>| ui.pause.lock().unwrap().as_deref() == m;
    engine.lock().unwrap().set_pause(&[], Some("Regardez le tableau.".into()), now_ms()).unwrap();
    until("écran en pause", || paused(Some("Regardez le tableau."))).await;
    engine.lock().unwrap().set_pause(&[], None, now_ms()).unwrap();
    until("écran repris", || paused(None)).await;
    engine.lock().unwrap().set_pause(&[], Some(String::new()), now_ms()).unwrap();
    until("écran en pause", || paused(Some("Écran mis en pause par Mme Martin."))).await;

    // Filtrage web (WEB-1) : le poste pose son proxy local ; un site
    // bloqué est refusé et l'élève en est prévenu (WEB-2).
    engine.lock().unwrap().set_web(&[], Some(WebPolicy::Allow { sites: vec!["education.gouv.fr".into()] }), now_ms()).unwrap();
    until("filtrage posé", || proxy.value.lock().unwrap().starts_with("pac ")).await;
    until("filtrage annoncé", || view(&engine).room.unwrap().stations[0].web_state == WebState::On).await;
    let pac = proxy.value.lock().unwrap().clone();
    let port: u16 = pac.trim_start_matches("pac http://127.0.0.1:").trim_end_matches("/proxy.pac").parse().unwrap();
    let mut c = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    c.write_all(b"CONNECT jeux.example:443 HTTP/1.1\r\n\r\n").await.unwrap();
    let mut answer = String::new();
    c.read_to_string(&mut answer).await.unwrap();
    assert!(answer.starts_with("HTTP/1.1 403"));
    until("élève prévenu", || ui.notices.lock().unwrap().iter().any(|n| n == "jeux.example n'est pas accessible pendant la séance de Mme Martin.")).await;
    assert_eq!(ui.restrictions.lock().unwrap().items, ["Web : seuls quelques sites sont ouverts"]);

    // L'élève coupe le filtrage : la console le signale (MODE-3).
    *proxy.value.lock().unwrap() = "direct".into();
    until("sortie signalée", || view(&engine).room.unwrap().stations[0].web_state == WebState::Escaped).await;
    assert!(view(&engine).alerts[0].text.contains("le filtrage web a été coupé"));
    *proxy.value.lock().unwrap() = pac;

    // Applications interdites (APP-2) : une application ouverte par
    // l'élève est fermée, et l'élève en est prévenu.
    let game = tmp.join("surveo-e2e-jeu");
    std::fs::copy("/bin/sleep", &game).unwrap();
    let out = std::process::Command::new("sh").arg("-c").arg(format!("'{}' 120 >/dev/null 2>&1 & echo $!", game.display())).output().unwrap();
    let pid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
    engine.lock().unwrap().set_banned_apps(&[], vec!["Surveo-E2E-Jeu".into()], now_ms()).unwrap();
    until("application fermée", || !std::path::Path::new(&format!("/proc/{pid}")).exists()).await;
    until("élève prévenu", || ui.notices.lock().unwrap().iter().any(|n| n == "Surveo-E2E-Jeu a été fermée par Mme Martin.")).await;

    // Applications ouvertes d'un poste (APP-1) : la console en reçoit la liste.
    let member = view(&engine).room.unwrap().stations[0].member.clone();
    engine.lock().unwrap().list_apps(&member).unwrap();
    until("applications reçues", || view(&engine).room.unwrap().stations[0].apps.is_some()).await;

    // Mode examen (EXAM-1 à EXAM-3) : l'élève le voit avec son heure de
    // fin ; sa liste d'autorisation remplace le filtrage, puis le rend.
    let exam_end = now_ms() + 20 * 60_000;
    engine.lock().unwrap().set_exam(Some((exam_end, vec!["moodle.example".into()])), now_ms()).unwrap();
    until("examen affiché", || ui.restrictions.lock().unwrap().exam_ends_at_ms.is_some_and(|t| t.abs_diff(exam_end) < 5_000)).await;
    assert_eq!(ui.restrictions.lock().unwrap().items[0], "Web : seuls les sites de l'examen sont ouverts");
    engine.lock().unwrap().set_exam(None, now_ms()).unwrap();
    until("examen levé", || ui.restrictions.lock().unwrap().exam_ends_at_ms.is_none()).await;
    assert_eq!(ui.restrictions.lock().unwrap().items[0], "Web : seuls quelques sites sont ouverts");

    // Clôture (SEANCE-5) : retour à l'état d'origine, confirmé au
    // récapitulatif (RESTR-5) ; plus aucune miniature ni restriction.
    engine.lock().unwrap().close_session(now_ms(), false).unwrap();
    until("indicateur retiré", || ui.session.lock().unwrap().is_none()).await;
    until("pause levée", || paused(None)).await;
    until("réglage d'origine rétabli", || *proxy.value.lock().unwrap() == "réglage d'origine").await;
    until("remise en état confirmée", || view(&engine).closure.unwrap().restored == ["poste 01"]).await;
    assert_eq!(view(&engine).room.unwrap().stations[0].state, StationState::On);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let n = thumbs.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(thumbs.load(Ordering::Relaxed), n, "des miniatures arrivent encore après la clôture");

    let _ = std::fs::remove_dir_all(&tmp);
}
