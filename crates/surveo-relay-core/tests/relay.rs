//! Relais réel sur l'interface locale, postes connectés par TLS et console
//! embarquée.

use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use surveo_identity::{Establishment, Identity, Role};
use surveo_net::{handshake, ClientWs, Refused, WsMessage};
use surveo_protocol::*;
use surveo_relay_core::{Frame, LocalConsole, Relay, RelayConfig};

fn info(label: &str) -> AgentInfo {
    AgentInfo { label: label.into(), os: "test".into(), version: "0".into(), kind: PosteKind::Salle, mode: DeployMode::SansAdmin }
}

async fn next_text(c: &mut LocalConsole) -> ToConsole {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), c.frames.recv()).await.expect("message attendu").unwrap() {
            Frame::Text(t) => return serde_json::from_str(&t).unwrap(),
            Frame::Binary(_) => continue,
        }
    }
}

async fn next_command(ws: &mut ClientWs) -> ToAgent {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap() {
            WsMessage::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            _ => continue,
        }
    }
}

#[tokio::test]
async fn routes_between_members_of_one_establishment() {
    let etab = Establishment::create("Lycée test");
    let trust = etab.trust();
    let relay = Relay::start(RelayConfig { identity: etab.issue("relais", Role::Relay), trust: trust.clone(), port: 0, discovery: None })
        .await
        .unwrap();
    let addr = format!("127.0.0.1:{}", relay.port());
    let mut console = relay.attach_console("console-1");
    assert_eq!(next_text(&mut console).await, ToConsole::Roster { agents: vec![] });

    // Un poste de la salle B204 se connecte : la salle vient de son certificat.
    let package = etab.package(&"b204".into(), "B204", PosteKind::Salle, vec![]);
    let agent = Identity::enroll_agent(&package, "poste-1").unwrap();
    let (mut ws, relay_peer) = handshake::connect(&addr, &agent, &trust, Some(&info("PC-01"))).await.unwrap();
    assert_eq!(relay_peer.role, Role::Relay);
    match next_text(&mut console).await {
        ToConsole::Online { agent } => assert_eq!((agent.member.as_str(), agent.room.as_deref()), ("poste-1", Some("b204"))),
        other => panic!("{other:?}"),
    }

    // Commande adressée à la salle, puis acquittement remonté à la console.
    let cmd = Command::Identify { number: "07".into(), seconds: 5 };
    console.sender.send(&FromConsole::Send { id: 9, to: Target::Room("b204".into()), cmd: cmd.clone() });
    assert_eq!(next_command(&mut ws).await, ToAgent::Command { from: "console-1".into(), id: 9, cmd });
    let ack = AgentMsg::Ack { id: 9, ok: true, detail: None };
    ws.send(WsMessage::Text(serde_json::to_string(&ack).unwrap().into())).await.unwrap();
    assert_eq!(next_text(&mut console).await, ToConsole::FromAgent { from: "poste-1".into(), msg: ack.clone() });

    // Une commande pour une autre salle n'arrive pas.
    console.sender.send(&FromConsole::Send { id: 10, to: Target::Room("c102".into()), cmd: Command::Screens { on: true } });
    console.sender.send(&FromConsole::Send { id: 11, to: Target::All, cmd: Command::Screens { on: false } });
    match next_command(&mut ws).await {
        ToAgent::Command { id, .. } => assert_eq!(id, 11),
    }

    // Miniatures : la console reçoit la dernière, préfixée par le poste.
    for i in 0..5u8 {
        ws.send(WsMessage::Binary(encode_agent_frame(BinKind::Thumbnail, &[i]).into())).await.unwrap();
    }
    let mut last = None;
    while last != Some(4) {
        match tokio::time::timeout(Duration::from_secs(5), console.frames.recv()).await.unwrap().unwrap() {
            Frame::Binary(b) => {
                let (kind, member, payload) = decode_console_frame(&b).unwrap();
                assert_eq!((kind, member), (BinKind::Thumbnail, "poste-1"));
                last = Some(payload[0]);
            }
            Frame::Text(t) => panic!("inattendu : {t}"),
        }
    }

    // Le poste se reconnecte : l'ancienne connexion est remplacée, sans
    // annonce de départ.
    let (mut ws2, _) = handshake::connect(&addr, &agent, &trust, Some(&info("PC-01"))).await.unwrap();
    assert!(matches!(next_text(&mut console).await, ToConsole::Online { .. }));
    drop(ws);
    ws2.send(WsMessage::Text(serde_json::to_string(&ack).unwrap().into())).await.unwrap();
    assert!(matches!(next_text(&mut console).await, ToConsole::FromAgent { .. }));
    assert_eq!(relay.agent_count(), 1);
    ws2.close(None).await.unwrap();
    assert_eq!(next_text(&mut console).await, ToConsole::Offline { member: "poste-1".into() });
}

#[tokio::test]
async fn refuses_a_member_of_another_establishment() {
    let etab = Establishment::create("A");
    let other = Establishment::create("B");
    let relay = Relay::start(RelayConfig { identity: etab.issue("relais", Role::Relay), trust: etab.trust(), port: 0, discovery: None })
        .await
        .unwrap();
    let addr = format!("127.0.0.1:{}", relay.port());
    let mut console = relay.attach_console("console-1");
    let _ = next_text(&mut console).await;

    // Poste d'un autre établissement : il ne reconnaît pas le relais.
    let package = other.package(&"x".into(), "X", PosteKind::Salle, vec![]);
    let intrus = Identity::enroll_agent(&package, "intrus").unwrap();
    let e = handshake::connect(&addr, &intrus, &other.trust(), Some(&info("X"))).await.unwrap_err();
    assert!(e.downcast_ref::<Refused>().is_some(), "{e:#}");

    // Même poste qui ferait confiance au relais : le relais le refuse et le signale.
    let e = handshake::connect(&addr, &intrus, &etab.trust(), Some(&info("X"))).await.unwrap_err();
    assert!(e.to_string().contains("autre établissement"), "{e:#}");
    match next_text(&mut console).await {
        ToConsole::Refused { reason, .. } => assert!(reason.contains("autre établissement"), "{reason}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(relay.agent_count(), 0);
}
