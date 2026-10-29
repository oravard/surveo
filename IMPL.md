# Surveo — réalisation

Ce document dit comment Surveo est réalisé : architecture, fichiers,
tests, mesures, limites. Un travail proposé ou en cours y garde son
ordre de réalisation ; une fois fait, il est décrit au présent. Le
comportement attendu est défini dans SPEC.md, auquel ce document renvoie
par les identifiants de règles.

## Ordre de réalisation

| Phase | Contenu | État |
|---|---|---|
| 0. Spécification | SPEC.md | Fait |
| 1. Maquette | `maquette/console-surveo.html`, validée (SPEC.md, 9.10) | Fait |
| 2. Prototypes techniques | Lever les risques techniques un par un (ci-dessous) | En cours |
| 3. Architecture | Composants, protocole, organisation du code, à partir des résultats de la phase 2 | À faire |
| 4. Développement | Par incréments complets, en commençant par une séance minimale | À faire |

### Phase 2 — prototypes techniques

Chaque prototype isole un risque et répond à une question précise, avec
un critère de réussite fixé à l'avance. Son code est jetable et vit dans
`prototypes/<nom>/`. Ses résultats (mesures, limites, décision) sont
consignés dans le chapitre de ce document qui correspond au thème de
SPEC.md concerné. Un prototype qui échoue remet en cause un choix
(Rust et Tauri, architecture du relais) avant qu'on ne bâtisse dessus.

| Ordre | Prototype | Question | Critère de réussite | SPEC.md | État |
|---|---|---|---|---|---|
| 1 | `relais` | Un relais en Rust tient-il une salle de 30 agents connectés en sortie, avec des miniatures et des commandes, en chiffrant et en authentifiant par la clé d'établissement ? | 30 agents simulés sur un portable ; une commande arrive en moins de 200 ms ; miniatures à 1 image/s soutenues ; découverte du relais par annonce sur le réseau local | Chap. 6, 7 | Fait ([7.1](#71-prototype-relais)) |
| 2 | `capture` | Peut-on capturer l'écran sans droits d'administration sous Windows, X11 et Wayland (GNOME, KDE, COSMIC), et à quel prix ? | Miniature à 1 image/s pour moins de 5 % d'un cœur ; sous Wayland, consentement demandé une seule fois puis mémorisé | Chap. 1 (AFF), 4 (DIFF) | À faire |
| 3 | `sans-admin` | Peut-on installer l'agent, le démarrer à l'ouverture de session, appliquer un proxy utilisateur et rétablir exactement le réglage d'origine, sans droits d'administration ? | Aucune élévation demandée ; réglage d'origine rétabli à l'identique ; liste des navigateurs qui respectent le proxy | Chap. 1 (RESTR-4), 2 (WEB, MODE), 7 (INST) | À faire |
| 4 | `tauri` | La console Tauri peut-elle embarquer le relais et afficher le plan alimenté par le prototype 1 ? Le panneau élève peut-il s'afficher dans une vue web côté agent ? | Console fonctionnelle sous Windows et Linux ; WebView2 disponible sans droits d'administration sur les postes Windows visés | Chap. 7 (RELAIS-1), 9 | À faire |
| 5 | `classifieur` | Le classifieur CamemBERT, converti en ONNX et quantifié, tourne-t-il sur un poste élève modeste depuis Rust ? | Moins de 150 Mo sur disque, moins de 300 Mo de mémoire, moins de 100 ms par phrase ; sinon, plan B avec un modèle distillé | Chap. 5 (IA-2, SENS-3) | À faire |
| 6 | `antivirus` | Comment Microsoft Defender et les antivirus courants des établissements réagissent-ils à l'agent, signé ou non ? | Aucun blocage d'un binaire signé ; sinon, liste des comportements en cause et des ajustements | Chap. 5, 7 | À faire |

Le prototype 1 vient en premier : il ne dépend d'aucune bibliothèque
système et sert de base aux prototypes 2 et 4. Les prototypes 3 et 6
gagnent à être menés sur de vrais postes de salle (par exemple un poste
géré par la Région), où l'on n'a justement pas les droits
d'administration.

### Environnement de test

- **Linux, Wayland** : poste de développement sous Fedora 44, session
  Wayland avec le bureau COSMIC. GNOME, KDE et une session X11 sont
  testés en machines virtuelles (VirtualBox et libvirt sont disponibles).
- **Windows** : machine virtuelle Windows 10 ou 11, et si possible un
  poste réel de salle sans droits d'administration.
- **Outils** : Rust 1.98 et Node 22 sont installés. Bibliothèques de
  développement à ajouter pour les prototypes 2 et 4 :
  `pipewire-devel`, `clang` (liaisons PipeWire), `webkit2gtk4.1-devel`,
  `gtk3-devel`, `openssl-devel`, `librsvg2-devel`,
  `libappindicator-gtk3-devel`, puis `cargo install tauri-cli`.

## 6. Rôles et confiance

### 6.1 Authentification mutuelle par la clé d'établissement

Éprouvée par le prototype `relais` (7.1), dans
`prototypes/relais/src/lib.rs`.

- **Clé d'établissement** : une paire de clés Ed25519 créée localement
  (CONF-1). Sa partie publique (identifiant et clé publique) est livrée
  avec le paquet d'installation (INST-3) ; sa partie privée reste chez le
  détenteur du rôle établissement.
- **Certificat de membre** : chaque relais, console ou agent a sa propre
  paire de clés Ed25519. L'établissement signe l'ensemble
  {établissement, membre, rôle, clé publique}.
- **Échange à la connexion**, après la négociation TLS :
  1. le relais envoie son certificat et un défi aléatoire ;
  2. le client vérifie le certificat du relais avec la clé publique de
     l'établissement, puis renvoie son certificat, son propre défi et sa
     signature du défi du relais ;
  3. le relais vérifie le certificat et la signature du client, puis
     signe le défi du client ;
  4. le client vérifie cette signature.
- **Liaison à la session TLS** : chaque signature porte aussi sur une
  valeur exportée de la session TLS (export de clé, étiquette
  `EXPORTER-surveo-proto-v0`). Un intermédiaire qui ouvrirait deux
  sessions TLS distinctes ne pourrait pas faire accepter une signature
  d'une session sur l'autre. Le certificat TLS du relais peut donc rester
  auto-signé : la confiance vient de la clé d'établissement, pas d'une
  autorité de certification.
- **Refus** : un membre d'un autre établissement, un certificat non
  signé par l'établissement ou une signature invalide entraînent un
  refus explicite, journalisé par le relais (CONF-3).

Non éprouvé à ce stade : le retrait d'une console (PAIR-5), le code
d'appairage (PAIR-1) et la conservation de la clé d'établissement.

## 7. Déploiement et relais

### 7.1 Prototype `relais`

Code dans `prototypes/relais/` (jetable). Mesure reproductible :
`./mesure.sh [agents] [durée en s]`.

**Composition**

| Binaire | Rôle |
|---|---|
| `keygen` | Crée un établissement de démonstration, les identités du relais, d'une console et de 300 agents, et 3 agents d'un autre établissement |
| `relay` | Relais : WebSocket sur TLS, authentification (6.1), annonce UDP sur le réseau local toutes les 2 s, relais des miniatures vers les consoles et des commandes vers les agents ; affiche son CPU et sa mémoire toutes les 5 s |
| `agent-sim` | N agents : trouvent le relais par son annonce, se connectent en sortie, envoient une miniature factice de 14 à 26 Ko par seconde, acquittent aussitôt chaque commande |
| `console-sim` | Console : envoie une commande par seconde à toute la salle, mesure l'aller-retour de chaque acquittement et le débit des miniatures, puis compare aux critères |

Choix techniques : Rust, tokio, WebSocket (tokio-tungstenite), TLS
(rustls avec le moteur *ring*), signatures Ed25519 (ed25519-dalek),
messages de contrôle en JSON, miniatures en trames binaires préfixées
par l'identifiant du poste.

**Mesures** (29 octobre 2026, Fedora 44, Intel Core i9-13900K, tout sur
la même machine)

| Scénario | Connexion et authentification | Aller-retour d'une commande | Miniatures | Relais |
|---|---|---|---|---|
| 30 agents, 30 s | Les 30 en 51 ms ; 1,6 ms par agent (médiane) | Médiane 0,4 ms, p95 0,6 ms | 30,0/s (0,62 Mo/s) | 0,2 % d'un cœur, 9 Mo |
| 300 agents, 20 s | Les 300 en 53 ms ; 19 ms par agent (médiane) | p95 2,0 ms, max 2,6 ms | 300/s (6,2 Mo/s) | 2 % d'un cœur, 57 Mo |

- Aucun acquittement manquant dans les deux scénarios.
- Les agents ont trouvé le relais par son annonce diffusée sur le réseau
  local (adresse réelle de la machine, pas seulement l'interface
  locale).
- Un agent de l'autre établissement est refusé avec un motif explicite
  (« membre d'un autre établissement »).

**Résultat** : critères atteints avec une marge d'au moins un facteur
10. L'architecture du chapitre 7 de SPEC.md (connexions en sortie vers
un relais, authentification par la clé d'établissement) est confirmée,
et un même relais peut servir plusieurs salles.

**Limites de la mesure**

- Tout tournait sur une seule machine puissante : la latence du réseau
  et un relais sur un portable modeste ne sont pas représentés. À
  refaire sur le réseau d'un établissement, avec le relais dans la
  console d'un portable (prototype `tauri`).
- L'annonce par diffusion ne franchit pas les réseaux séparés (VLAN) :
  attendu, c'est le rôle des autres niveaux de découverte (DECOUV-1).
- Miniatures factices en octets aléatoires ; leur taille réelle viendra
  du prototype `capture`.

**À reprendre dans l'implémentation**

- Canaux bornés entre le relais et chaque client : une console lente ne
  doit pas faire grossir la mémoire du relais. Pour les miniatures, ne
  garder que la plus récente de chaque poste.
- Plusieurs établissements sur un même relais hébergé (RELAIS-3) et
  retrait d'une console (PAIR-5), non traités par le prototype.
- Le relais voit le contenu qu'il relaie : la question du chiffrement de
  bout en bout (SPEC.md, 6.10) reste ouverte.
