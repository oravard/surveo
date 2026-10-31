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
| 2. Prototypes techniques | Lever les risques techniques un par un (ci-dessous) | Fait |
| 3. Architecture | Composants, protocole, organisation du code, à partir des résultats de la phase 2 | Validée ([Architecture](#architecture)) ; questions ouvertes reportées au développement |
| 4. Développement | Par incréments complets, en commençant par une séance minimale | En cours : tranches 1 à 3 faites ([1.2](#12-séance-minimale), [2.2](#22-maîtrise-en-séance), [3.1](#31-pédagogie-en-séance)) |

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
| 2 | `capture` | Peut-on capturer l'écran sans droits d'administration sous Windows, X11 et Wayland (GNOME, KDE, COSMIC), et à quel prix ? | Miniature à 1 image/s pour moins de 5 % d'un cœur ; sous Wayland, consentement demandé une seule fois puis mémorisé | Chap. 1 (AFF), 4 (DIFF) | Fait ([1.1](#11-prototype-capture)) : Windows, X11, Wayland COSMIC, GNOME et KDE |
| 3 | `sans-admin` | Peut-on installer l'agent, le démarrer à l'ouverture de session, appliquer un proxy utilisateur et rétablir exactement le réglage d'origine, sans droits d'administration ? | Aucune élévation demandée ; réglage d'origine rétabli à l'identique ; liste des navigateurs qui respectent le proxy | Chap. 1 (RESTR-4), 2 (WEB, MODE), 7 (INST) | Fait ([2.1](#21-prototype-sans-admin)) |
| 4 | `tauri` | La console Tauri peut-elle embarquer le relais et afficher le plan alimenté par le prototype 1 ? Le panneau élève peut-il s'afficher dans une vue web côté agent ? | Console fonctionnelle sous Windows et Linux ; WebView2 disponible sans droits d'administration sur les postes Windows visés | Chap. 7 (RELAIS-1), 9 | Fait ([7.2](#72-prototype-tauri)) : pare-feu à confirmer avec un compte standard |
| 5 | `classifieur` | Le classifieur CamemBERT, converti en ONNX et quantifié, tourne-t-il sur un poste élève modeste depuis Rust ? | Moins de 150 Mo sur disque, moins de 300 Mo de mémoire, moins de 100 ms par phrase ; sinon, plan B avec un modèle distillé | Chap. 5 (IA-2, SENS-3) | Fait ([5.1](#51-prototype-classifieur)) |
| 6 | `antivirus` | Comment Microsoft Defender et les antivirus courants des établissements réagissent-ils à l'agent, signé ou non ? | Aucun blocage d'un binaire signé ; sinon, liste des comportements en cause et des ajustements | Chap. 5, 7 | Conclu pour l'essentiel ([7.3](#73-prototype-antivirus)) : Defender ne bloque aucun comportement ; décision en SPEC INST-7 ; vrai certificat à vérifier au jalon crédibilité |

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

## Architecture

Ce chapitre fixe l'architecture globale à partir des résultats de la
phase 2 : les composants, le protocole qui les relie, où vit l'état, et
l'organisation du code. C'est une **proposition datée (30 octobre 2026),
à valider**, qui sert de base à la phase 4 (développement). Les règles de
comportement sont dans SPEC.md, auquel ce chapitre renvoie par leurs
identifiants ; les choix techniques validés le sont par les prototypes de
la phase 2 (chapitres 1 à 7 de ce document).

### Vue d'ensemble

Trois programmes et des bibliothèques partagées, tous en Rust ; toutes
les interfaces en TypeScript (VISION.md §8).

```
   poste élève            poste élève              poste enseignant
  ┌───────────┐          ┌───────────┐            ┌──────────────────┐
  │  agent    │          │  agent    │            │  console (Tauri) │
  │ + panneau │   …      │ + panneau │            │  + relais (opt.) │
  └─────┬─────┘          └─────┬─────┘            └─────────┬────────┘
        │ sortie TLS           │ sortie TLS                 │ sortie TLS
        └──────────────┬───────┴────────────────────────────┘
                       ▼
                 ┌───────────┐   un relais par établissement, à l'un de
                 │  relais   │   trois emplacements : dans la console,
                 └───────────┘   sur un serveur du lycée, ou hébergé (FR)
```

- **Agent** (poste élève) : programme Rust, sans fenêtre, plus un
  **panneau élève** en vue web (AGENT-3, prototype `tauri`). Se connecte
  **en sortie** au relais (RELAIS-4), n'écoute jamais le réseau (7.2,
  enseignement 4). Il capture l'écran (prototype `capture`), applique les
  restrictions (prototype `sans-admin`), exécute les classifieurs locaux
  (prototype `classifieur`), et affiche à l'élève le TP, les messages, les
  sondages et la demande d'aide.
- **Console** (enseignant) : application **Tauri 2** (cœur Rust, interface
  TypeScript, prototype `tauri`). Elle peut **embarquer le relais**
  (RELAIS-1, relais dans la classe) via `relay_core`. Elle porte le plan
  de salle, les miniatures, et conduit la séance.
- **Relais** : le **même** programme Rust aux trois emplacements
  (RELAIS-1, RELAIS-2). Il route entre membres d'un même établissement
  (CONF-4), n'accepte que des connexions en sortie, authentifie par la
  clé d'établissement (6.1), garde seulement la dernière miniature de
  chaque poste et borne ses files (7.1, « à reprendre »).
- **Bibliothèques partagées** : voir « Organisation du code ».

### Protocole

- **Transport** : WebSocket sur TLS (`tokio` + `tokio-tungstenite` +
  `rustls`/*ring*), éprouvé par le prototype `relais` (7.1). Le certificat
  TLS du relais peut rester auto-signé : la confiance vient de la clé
  d'établissement, pas d'une autorité (6.1).
- **Authentification** : échange mutuel par certificats de membre signés
  par la clé d'établissement Ed25519, lié à une valeur exportée de la
  session TLS (6.1). Un membre d'un autre établissement est refusé et
  journalisé (CONF-3).
- **Deux types de trames** :
  - **messages de contrôle** typés, sérialisés par `serde` (JSON au
    prototype ; choix du format compact à trancher, voir questions
    ouvertes), avec un **champ de version** du protocole ;
  - **trames binaires** pour le volumineux (miniatures, diffusion
    d'écran, transfert de fichiers), préfixées par l'identifiant du poste
    (7.1). Pour les miniatures, le relais ne conserve que la plus récente
    par poste.
- **Adressage** : une commande vise la séance entière, une sélection de
  postes, ou un seul poste (prototype `tauri`). Les retours (aide, rendus,
  réponses aux sondages, « traité ») remontent des agents vers les
  consoles.
- **Acquittements** : les commandes de restriction sont acquittées poste
  par poste (RETOUR-1, mesuré au prototype `relais`). Les miniatures sont
  au mieux (dernière connue). Les rendus sont fiables, avec reprise
  (RENDU-3).
- **Report pendant une déconnexion** : un message, un TP ou un fichier
  destiné à un poste momentanément absent lui parvient à son retour, tant
  que la séance est ouverte (MSG-5, TP-2, DISTRIB-4). Voir « État et
  stockage » pour savoir qui garde ces éléments en attente.
- **Familles de messages** (dérivées de SPEC) : présence et état des
  postes (SALLE-1, MODE-2) ; miniatures (chap. 1, AFF) ; restrictions
  (chap. 2 : pause, filtrage, fermeture d'application, mode examen) ;
  pédagogie (chap. 3 : TP, étapes, rendus, distribution, sondages) ;
  communication (chap. 4 : messages, conversation, aide, diffusion) ;
  configuration poussée signée (chap. 7, RECONF-1) ; appairage (6.5) ;
  mises à jour signées (7.6) ; entrées du journal d'audit (6.7).
- **Chiffrement de bout en bout** : objectif, non tenu en v1 (6.10,
  HEBERG-3). En v1, le relais voit ce qu'il route ; la confidentialité
  repose sur TLS de bout à bout de chaque lien et sur la confiance
  d'établissement.

### État et stockage

Principe directeur, issu du chapitre 8 : **le relais voit et garde le
moins possible** ; l'état durable est du côté console/établissement ;
l'agent est **autonome** pendant une coupure.

- **Agent** (poste) : garde localement de quoi tenir une déconnexion —
  son identité (certificat de membre + clés), son rattachement, le dernier
  relais et la dernière configuration reçus, le **journal de remise en
  état** (valeurs d'origine à restaurer, prototype `sans-admin`), le jeton
  de restauration de la capture, et l'état de son TP en cours (étapes
  terminées, rendus envoyés, rendus en attente d'envoi). Cela satisfait
  RESTR-2, RESTR-3, TP-3 et RENDU-3 sans que le relais ait à mémoriser
  l'état du poste. Rangé dans le dossier utilisateur
  (`~/.local/share/surveo/agent`, `%LOCALAPPDATA%\Surveo\Agent`), avec
  l'exécutable installé sans droits d'administration ; la console range
  ses fichiers à côté (`…/console`, `…\Console`).
- **Console responsable** (celle qui a ouvert la séance, MULTI-2) : tient
  l'état **autoritatif de la séance** — postes joints, noms de la séance,
  restrictions et TP appliqués, ordre de la file d'aide (AIDE-2), rendus
  reçus. Un poste qui se reconnecte se resynchronise depuis elle, via le
  relais.
- **Relais** : route et ne bufferise qu'au minimum (dernière miniature par
  poste ; au besoin une courte file bornée de messages de séance pour un
  poste brièvement absent). Il ne persiste pas le contenu.
- **Données durables** (chap. 8, DATA-1) : rendus de TP rangés côté
  enseignant par séance → TP → poste (RENDU-6) ; configuration (salles,
  plans, listes, rattachements), journal d'audit et traçabilité
  éventuelle côté établissement/relais. Suppression automatique à
  l'échéance (CONSERV-3).

### Dimensionnement

Chiffres tirés des prototypes `relais` (7.1) et `capture` (1.1). Tout le
trafic vient des **miniatures** ; présence, commandes et acquittements
sont négligeables. Rappel : l'image d'écran est **désactivée par défaut**
(AFF-4), elle ne circule que quand l'enseignant affiche les écrans.

Miniatures réelles mesurées : 1,5 à 8 Ko (≈ 8 Ko au pire, GNOME). Pour
une classe de 30 postes à 1 image/s, écrans affichés :

| Flux | Débit |
|---|---|
| Montée d'un agent vers le relais | ~8 Ko/s ≈ 64 kbit/s |
| 30 agents → relais | ~240 Ko/s ≈ 1,9 Mbit/s |
| Relais → une console | ~240 Ko/s ≈ 1,9 Mbit/s |
| Total vu par le relais (1 classe, 1 console) | ~0,5 Mo/s ≈ 3,8 Mbit/s |
| Plan de salle sans écrans (défaut) | quasi nul |

Chaque console supplémentaire sur la séance (MULTI-1) ajoute autant en
sortie du relais.

**Le relais est léger en CPU et en mémoire** : le prototype a tenu 300
agents (≈ 10 classes) à 1 image/s avec des miniatures factices de 14 à
26 Ko, pour 6,2 Mo/s, **2 % d'un seul cœur** et **57 Mo** de RAM, sans
acquittement manquant. Avec les miniatures réelles (~8 Ko), ~2,4 Mo/s
pour 300 agents. Le facteur d'échelle d'un **relais hébergé** n'est donc
pas le CPU mais la **bande passante** : ~10 classes écrans affichés
≈ ~40 Mbit/s agrégés, qu'un serveur absorbe aisément et qu'on peut
répartir par établissement (RELAIS-3, VISION.md §6).

**À mesurer avant de conclure** : la **diffusion d'écran** (DIFF-1, une
source ré-émise vers 30 postes) et le **mode focus** (AFF-3, image plus
grande et plus rapide à la demande) sont les opérations les plus lourdes
et ne sont pas encore mesurées. Les chiffres ci-dessus ont été pris sur
une seule machine, sans latence ni pertes de réseau réelles (7.1).

### Identité et confiance

- **Identité en couches** (chap. 1, VISION.md §8) : poste (socle,
  toujours présent) → place dans la salle (plan) → séance → élève
  (facultatif, par séance).
- **Clé d'établissement** Ed25519 créée localement (CONF-1) ; chaque
  relais, console et agent a sa paire de clés et un **certificat de
  membre** {établissement, membre, rôle, clé publique} signé par
  l'établissement (6.1).
- **Appairage** d'une console par code court et temporaire, via le relais,
  hors ligne (PAIR-1 à PAIR-4). **Retrait** d'une console (PAIR-5) :
  l'établissement publie une liste de révocation signée, poussée dans la
  configuration ; un poste n'obéit plus à une console révoquée (CONF-3).
- **Rôles** (enseignant, technicien, établissement) portés par le
  certificat de membre ; la console n'expose que ses rôles (ROLE-3), le
  poste ne vérifie l'autorisation qu'à l'action (CONF-3).
- **Configuration et mises à jour signées** (RECONF-1, MAJ-2) : un relais,
  même compromis, ne peut pas pousser de code ni de configuration non
  signés.

### Capacités par plateforme

Un module d'abstraction par système, derrière des traits Rust, avec une
implémentation par OS (éprouvées par les prototypes `capture` et
`sans-admin`) :

- **Capture** : Wayland (portail ScreenCast + PipeWire, jeton de
  restauration ; cadence à la demande sous GNOME/KDE, flux suspendu sous
  COSMIC — 1.1), X11 (`GetImage`), Windows (`xcap`, puis API de
  duplication du bureau à étudier). Un seul écran en v1.
- **Restrictions** : filtrage par proxy de l'utilisateur (WinINet sous
  Windows, `gsettings` sous Linux ; mode PAC sans droits d'administration,
  2.1), démarrage de session (clé `Run` / `~/.config/autostart`), écran en
  pause (fenêtre de recouvrement), fermeture d'application. Chaque action
  relève la valeur d'origine et la restaure (RESTR-4).
- **Modes de déploiement** (MODE-1 à MODE-6) : la même base, mais sans
  droits d'administration le filtrage cesse avec l'agent (PAC) et ne porte
  que sur les navigateurs ; la console affiche le mode de chaque poste.

### IA sur le poste

Éprouvé par le prototype `classifieur` (5.1) :

- **Classifieurs locaux** exécutés sur le poste par ONNX Runtime depuis
  Rust (IA-2, SENS-3) ; le modèle n'est pas livré avec l'agent, il est
  téléchargé depuis le relais à l'activation du module et chargé
  seulement tant qu'il est actif (SENS-2). Rien n'est exfiltré.
- **IA générative** : les requêtes passent par le relais de l'enseignant,
  qui choisit la destination (local, Mistral, Google… — SPEC 5.6).

### Organisation du code

Un espace de travail Cargo à la racine du dépôt ; les prototypes
(`prototypes/`) en sont exclus. Les applications réutilisent des crates
communes :

| Crate | Dossier | Rôle |
|---|---|---|
| `surveo-protocol` | `crates/` | Messages de contrôle et trames binaires, version du protocole. Partagée par tous. |
| `surveo-identity` | `crates/` | Clé d'établissement, certificats de membre, paquet de rattachement (6.1). |
| `surveo-net` | `crates/` | WebSocket sur TLS, authentification mutuelle liée à la session TLS, recherche du relais sur le réseau local. |
| `surveo-relay-core` | `crates/` | Moteur du relais, autonome ou embarqué dans la console (7.4). |
| `surveo-platform` | `crates/` | Capacités par OS : capture d'écran, démarrage de session, proxy de l'utilisateur, applications de la session, fichiers de l'utilisateur (Documents, ouverture), dossiers. |
| `surveo-agent` | `apps/agent/` | Agent du poste élève : un cœur sans interface (`lib`) et l'application Tauri (`main`). |
| `surveo-console` | `apps/console/` | Console : un cœur sans interface (`lib`), l'application Tauri qui embarque le relais (`main`). |

Le binaire relais autonome (relais d'établissement ou hébergé) viendra
avec l'étape 8 ; le moteur est déjà le même.

- **Interfaces** : Svelte 5 et TypeScript, construites par Vite, dans
  `ui/` : un seul projet npm, deux interfaces (`ui/console`,
  `ui/agent`) et une partie commune (`ui/shared` : charte, icônes de la
  maquette, polices livrées avec l'application). `npm run build` produit
  `ui/dist/console` et `ui/dist/agent`, que lisent les `tauri.conf.json` ;
  il faut donc construire les interfaces avant de compiler les
  applications.
- **Types partagés Rust ↔ TypeScript** : engendrés par `ts-rs` dans
  `ui/shared/bindings/` (`cargo test export_bindings`, pour la console et
  le panneau élève), et versionnés.
- **Construction** : `scripts/build.sh` construit les interfaces, puis
  l'agent et la console pour Linux et pour Windows (compilés depuis
  Fedora, cible `x86_64-pc-windows-gnu`), et assemble dans `dist/` une
  archive par système, avec la notice d'essai (`scripts/LISEZMOI.txt`)
  et les empreintes SHA-256.

### Ordre de réalisation (phase 4)

**Principe** : chaque incrément livre une **version opérationnelle et
autonome** — agent + console + relais — ne contenant que les
fonctionnalités déjà développées, mais installable et testable telle
quelle, y compris sur un **réseau local sans aucun serveur** grâce au
relais embarqué dans la console (RELAIS-1). C'est ainsi qu'on valide
chaque étape dans de vrais environnements (les salles en réseau local de
l'utilisateur, où il est administrateur). Chaque incrément est vertical
(du poste à la console) et s'achève par un build des trois composants.
Les premiers builds peuvent être non signés — testables parce que
l'utilisateur est administrateur de ses postes ; la signature est
formalisée à l'étape 8 (INST-7).

1. **Séance minimale (et socle)** — fait ([1.2](#12-séance-minimale)) :
   mettre en place le workspace et les
   crates communes (`surveo-protocol`, `surveo-identity`, `relay_core`
   promu hors des prototypes `relais`/`tauri`), puis livrer la première
   tranche utilisable : l'agent se connecte et se place, la console (avec
   relais embarqué) montre le plan et les miniatures, on ouvre, prolonge
   et clôture une séance, et le poste revient à son état d'origine
   (chap. 1). **Premier trio autonome testable en salle.**
2. **Maîtrise** — fait ([2.2](#22-maîtrise-en-séance)) : écran en
   pause, filtrage web, fermeture d'application, mode examen, via
   `surveo-platform` (chap. 2).
3. **Pédagogie** — fait ([3.1](#31-pédagogie-en-séance)) : modèles de
   TP, lancement et suivi, rendus, distribution, sondages (chap. 3).
4. **Communication** : messages, conversation, demande d'aide, diffusion
   d'écran (chap. 4).
5. **Rôles et confiance** : appairage, rôles, révocation, journal
   d'audit ; configuration poussée signée ; mises à jour signées
   (chap. 6, 7).
6. **IA** : classifieurs locaux, IA générative via le relais (chap. 5).
7. **Données et RGPD** : conservation, traçabilité en option, kit RGPD
   (chap. 8).
8. **Durcissement et diffusion** : signature du code et soumission aux
   éditeurs d'antivirus (INST-7, 7.3), paquets d'installation, relais
   d'établissement et hébergé, deux modes de déploiement.

À chaque incrément à partir du premier, le résultat est un jeu
agent/console/relais déployable sur une salle pour l'éprouver.

### Décisions

30 octobre 2026 : architecture proposée et validée ; les questions
ouvertes ci-dessous sont reportées au développement (leur impact
s'éclaircira en avançant).

- Trois programmes Rust (agent, console Tauri, relais), interfaces en
  TypeScript ; le relais est le même binaire aux trois emplacements.
- Transport WebSocket/TLS, authentification par la clé d'établissement
  (6.1), confirmés par les prototypes `relais` et `tauri`.
- Contrôle en messages typés `serde` versionnés ; volumineux en trames
  binaires préfixées par le poste.
- **Relais mince** : l'état durable est côté console/établissement,
  l'agent est autonome pendant une coupure ; le relais route et garde le
  minimum. Cohérent avec l'objectif de chiffrement de bout en bout (6.10).
- Capacités système derrière un trait par OS (`surveo-platform`).
- Types Rust ↔ TypeScript engendrés depuis Rust.
- Développement phase 4 par tranches verticales, à partir d'une séance
  minimale.
- Chaque incrément de la phase 4 est une **version opérationnelle
  autonome** (agent + console + relais embarqué), testable en salle sur
  réseau local sans serveur, ne contenant que les fonctionnalités déjà
  faites (demande de l'utilisateur, 30 octobre 2026).
- 31 octobre 2026, au début de la tranche 1 : interfaces en **Svelte 5**
  et TypeScript, construites par Vite (choix de l'utilisateur) ; types
  engendrés par **ts-rs** ; messages de contrôle en **JSON** tant que le
  débit ne le remet pas en cause (les miniatures, seul flux important,
  sont en binaire).
- 31 octobre 2026, à la tranche 3 : transferts de fichiers tirés par le
  destinataire, morceau par morceau ; tailles maximales, retrait des
  fichiers distribués modifiés, ouverture du panneau élève et sondage
  unique confirmés par l'utilisateur (SPEC.md, 3.9).

### Questions ouvertes

- Format des messages de contrôle : JSON en tranche 1 ; passer à un
  format compact (CBOR/MessagePack via `serde`) si une mesure le
  justifie.
- Report pendant une déconnexion : la courte file de messages de séance
  est-elle tenue par le relais ou seulement par la console responsable ?
  Pour les restrictions, le TP, les fichiers et le sondage, la console
  renvoie un état complet à la reconnexion (2.2, 3.1) ; reste la
  question pour les messages du chapitre 4.
- Chiffrement de bout en bout consoles ↔ postes : quelle portée en v1,
  et que doit encore voir le relais pour router (6.10, 8.12) ?
- Ce qui arrive à la séance si la console responsable se ferme en cours
  (reprise par une autre console autorisée ?).

## 1. Séance et salle

### 1.1 Prototype `capture`

Code dans `prototypes/capture/` (jetable). Une même chaîne pour toutes
les plateformes : image brute de l'écran → réduction (fast_image_resize)
→ JPEG (jpeg-encoder), avec la mesure de chaque étape et du CPU. Trois
sources d'images :

| Plateforme | Source | Accord de l'utilisateur |
|---|---|---|
| Wayland | Portail ScreenCast (ashpd), puis flux PipeWire (pipewire-rs) | Demandé par le portail ; jeton de restauration conservé dans `~/.config/surveo-proto/` |
| X11 | Lecture de l'écran racine (x11rb, `GetImage`) | Aucun |
| Windows | Bibliothèque xcap, écran principal | Aucun |

Commande : `surveo-proto-capture [--backend wayland|x11|windows]
[--duration 30] [--width 320] [--continu] [--oublier-jeton]`. La version
Windows est compilée depuis Fedora (cible `x86_64-pc-windows-gnu`,
MinGW) ; elle ne dépend que de bibliothèques présentes sur tout
Windows 10 ou 11.

**Mesures** (29 octobre 2026, miniature de 320 pixels de large, 1 image/s)

| Plateforme | Écran | Par miniature (médiane) | Taille | CPU de l'agent | Coût ajouté au compositeur |
|---|---|---|---|---|---|
| Wayland, COSMIC 1.9 (Fedora 44) | 5120×1440 | réduction 4,4 ms, JPEG 0,2 ms | 4,4 Ko | 0,55 % d'un cœur | environ 1 % d'un cœur |
| Wayland, GNOME 50 (Fedora 44, VM) | 1932×1047 | réduction 1,3 ms, JPEG 0,3 ms | 8,2 Ko | 0,2 % d'un cœur | environ 2 % d'un cœur |
| Wayland, KDE Plasma 6 / KWin 6.7.5 (Ubuntu 26.10, VM) | 2009×1120 | réduction 1,2 ms, JPEG 0,3 ms | 6,5 Ko | 0,2 % d'un cœur | environ 2 % d'un cœur |
| X11 (serveur virtuel Xvfb) | 1920×1080 | lecture 5,2 ms, réduction 1,1 ms | 1,5 Ko | 0,72 % d'un cœur | sans objet |
| Windows (compte administrateur, confirmé avec un compte standard) | 2300×1200 | lecture 37,8 ms, réduction 1,5 ms | 5,2 Ko | 1,77 % d'un cœur | non mesuré |

**Consentement sous Wayland** : au premier lancement, le portail COSMIC
affiche sa fenêtre d'accord ; il délivre alors un jeton de restauration.
Aux lancements suivants, l'accord est obtenu en 3 à 4 ms, sans fenêtre.
Sous GNOME 50 (portail version 5, VM Fedora 44) et sous KDE Plasma 6 /
KWin 6.7.5 (portail version 5, VM Ubuntu 26.10), même résultat : l'accord
est demandé une fois, puis le jeton de restauration est réutilisé sans
fenêtre (28 ms sous GNOME, 26 ms sous KDE, au deuxième lancement). Le
jeton honoré par KWin était le point le plus incertain : il fonctionne.
Le critère « accord demandé une seule fois » est atteint sous COSMIC,
GNOME et KDE.

**Le coût réel est chez le compositeur.** COSMIC n'accepte qu'une
cadence fixe de 60 images/s : une demande de 1 image/s au plus fait
échouer la négociation. Laissé ouvert, le flux lui fait copier l'écran
environ 47 fois par seconde : son CPU passe de 5 % à 33 % d'un cœur,
alors que celui de l'agent reste à 0,6 %. La parade retenue :
**suspendre le flux après chaque miniature et le réactiver à la
seconde suivante** (`set_active` de PipeWire). Le compositeur n'envoie
plus alors que 1,0 image/s, pour environ 1 % d'un cœur. La première
image livrée après le réveil est toujours prise : sinon le flux reste
ouvert de plus en plus longtemps à chaque cycle.

**GNOME et KDE, eux, négocient une cadence à la demande** (format
`0/1 image/s`) : le compositeur n'envoie que les images demandées, et la
suspension du flux n'est donc pas nécessaire (elle reste sans effet
néfaste). Le coût du compositeur pour 1 image/s est d'environ 2 % d'un
cœur (`gnome-shell` comme `kwin_wayland`), l'agent restant à 0,2 %. Seul
COSMIC impose pour l'instant la cadence fixe qui oblige à suspendre le
flux.

**Résultat** : critère atteint sous Wayland (COSMIC, GNOME et KDE), X11
et Windows ; la capture d'une miniature par seconde coûte au plus environ
2,5 % d'un cœur, compositeur compris. Les miniatures réelles pèsent 1,5 à 5 Ko, bien
moins que les 14 à 26 Ko supposés par le prototype `relais` (7.1).

**Limites de la mesure**

- Jeton de restauration vérifié sous COSMIC, GNOME et KDE.
- Mesures GNOME et KDE faites en VM, sur des écrans d'environ 2000×1100
  (plus petits que l'écran COSMIC réel de 5120×1440) : la réduction y est
  logiquement plus rapide. À confirmer sur un écran de pleine taille.
- Le même binaire, compilé sous Fedora 44, tourne tel quel sous Ubuntu
  26.10 (glibc 2.44, `libpipewire-0.3` présent) : il ne dépend que de
  `libpipewire` et de la glibc. Bon signe pour la distribution Linux.
- Sous XWayland, l'écran racine n'est pas lisible (erreur `BadMatch`) :
  attendu, puisque les fenêtres Wayland n'y figurent pas. Le chemin X11
  a été mesuré sur un serveur virtuel, sans carte graphique ; un vrai
  poste X11 reste à mesurer.
- Sous Windows, la mesure détaillée a été faite avec un compte
  administrateur ; un essai avec un compte standard, en session locale,
  donne des performances similaires. Le coût ajouté au gestionnaire de
  fenêtres (DWM) n'est pas mesuré ; la lecture de l'écran (38 ms) pourrait être réduite avec
  l'API de duplication du bureau.
- Un seul écran capturé (l'écran principal) ; les flux PipeWire en
  DMA-BUF ne sont pas traités (COSMIC a fourni une mémoire partagée).

**Premier constat pour le prototype `antivirus`** : sur le poste
Windows de test, l'antivirus (Avast) juge `capture.exe`, non signé,
suspect dès son lancement ; au bout de 50 secondes, son analyse n'a
toujours pas abouti. Sur un poste de salle sans droits
d'administration, personne ne pourrait passer outre : la signature du
code et la réputation de l'exécutable auprès des éditeurs
d'antivirus sont une condition du déploiement, pas un confort.

**À reprendre dans l'implémentation**

- Suspendre le flux PipeWire entre deux miniatures, et ne mesurer le
  coût d'une capture que compositeur compris.
- Conserver le jeton de restauration ; prévoir le cas où le portail
  n'en délivre pas, ou où l'utilisateur retire l'accord (la console doit
  alors afficher que l'écran du poste n'est pas visible).
- Vue en grand (AFF-3, mode focus) : plus grande taille et plus forte
  cadence à la demande, puis retour à la miniature.

### 1.2 Séance minimale

Première tranche de la phase 4 : un trio agent, console et relais intégré,
installable et testable en salle sur un réseau local sans serveur. Code
dans `apps/` et `crates/` (organisation : « Architecture »).

**Règles couvertes** : SALLE-1 à SALLE-4 ; AFF-1, AFF-2 (état et
libellé), AFF-4 ; SEANCE-1, SEANCE-3 à SEANCE-6 ; REJ-1, REJ-4 ; RESTR-1
à RESTR-3, RESTR-5 ; AGENT-1, AGENT-2 ; SEL-1, SEL-2, SEL-5 ; RETOUR-1,
RETOUR-3 à RETOUR-5 ; CONF-1, CONF-3 ; INST-1, INST-3, INST-5, INST-6 ;
DECOUV-1 (niveaux 1 et 3), DECOUV-2 ; RELAIS-1, RELAIS-4. Hors de la
tranche : poste personnel (REJ-2, REJ-3, REJ-5, PERSO), noms d'élèves
(ELEVE), vues liste, mosaïque et focus, plusieurs consoles (SEANCE-2,
MULTI), affichage du mode de déploiement (MODE-2), détection du pare-feu
(RELAIS-6).

**La console tient la séance.** Son cœur (`apps/console/src/engine.rs`)
est synchrone et sans interface : il reçoit les messages du relais, les
gestes de l'enseignant et l'heure courante, et envoie ses commandes par
le relais. L'application Tauri l'appelle et publie à l'interface l'état
complet à chaque changement.

- Ouvrir : la salle choisie et une heure de fin, au plus 4 h plus tard ;
  une seule séance à la fois. La séance est enregistrée
  (`seance.json`) : une console relancée la reprend, sauf si son
  échéance est passée.
- Chaque poste qui se connecte est ajouté à sa salle s'il est nouveau,
  avec le numéro suivant, dans « postes à placer ». S'il appartient à la
  salle de la séance, la console lui envoie aussitôt la séance, et les
  écrans s'ils sont affichés (REJ-1, RESTR-2, RESTR-3).
- Prolonger : nouvelle heure de fin, renvoyée à toute la salle, bornée à
  4 h après l'ouverture ; « Annuler » renvoie l'ancienne heure.
- Clôturer, ou fin d'elle-même à l'heure de fin plus 10 min : la
  commande de fin part vers toute la salle ; le récapitulatif liste les
  postes entrés dans la séance. Un poste connecté est « remis en état »
  quand il annonce n'être plus dans la séance, « injoignable » au bout
  de 5 s sans réponse ; un poste injoignable qui revient (redémarré, ou
  encore dans la séance close, à qui la console renvoie alors la fin)
  passe à « remis en état ».
- Retour poste par poste (RETOUR-1) : une action garde l'identifiant de
  chaque commande envoyée ; au bout de 5 s ou de toutes les réponses, la
  console affiche « Identifier : envoyé à 23 postes · 1 injoignable
  (poste 07) ».

Durées (`apps/console/src/lib.rs`) : séance d'1 h par défaut, 4 h au
plus, marge de 10 min, prolongation de 15 min. Ce sont les propositions
de SPEC.md, 1.11, encore ouvertes.

**L'agent est autonome** (`apps/agent/src/core.rs`). Il démarre toujours
hors séance (RESTR-3). La séance lui arrive avec la durée restante, pas
une heure : il calcule son échéance sur sa propre horloge, insensible à
un décalage avec la console. Il la quitte de lui-même à l'heure de fin
plus la marge, connecté ou non (RESTR-1, RESTR-2). Il ne capture l'écran
que dans une séance dont l'enseignant affiche les écrans, et cesse dès
que la connexion tombe. Il annonce son état (séance, écran) à la
connexion et à chaque changement ; la console s'en sert pour se
resynchroniser. Les restrictions et le journal de remise en état sont
décrits en 2.2.

**Messages** (`crates/surveo-protocol`) :

| Sens | Messages |
|---|---|
| Console → postes | `Session` (séance, enseignant, durée restante, marge), `EndSession`, `Screens` (afficher ou non), `Identify` (numéro, durée) ; chacun avec un identifiant et une adresse : toute la salle, ou des postes |
| Poste → consoles | `Ack` (réponse à une commande), `Status` (séance, état de l'écran : éteint, en attente, affiché, impossible avec son motif) |
| Relais → console | `Roster` (postes connectés), `Online` (poste, salle tirée de son certificat, description), `Offline`, `FromAgent`, `Refused` (connexion refusée et son motif) |
| Poste → relais → console, binaire | miniature JPEG, préfixée par sa nature et, vers la console, par le poste |

**Interface de la console** (`ui/console`) :

- Premier lancement : créer l'établissement (son nom, celui de
  l'enseignant tel que les élèves le verront). Rejoindre un
  établissement par appairage viendra avec le chapitre 6.
- Barre du haut : la salle (et « Nouvelle salle… ») ; « Ouvrir une
  séance », ou la séance en cours avec son heure de fin, le temps
  restant, « Prolonger » et « Clôturer » (confirmation qui énonce la
  portée, RETOUR-4).
- Plan : une grille de places (colonnes × rangées, réglable). On place
  un poste en le glissant, ou en cliquant le poste puis sa place ; une
  place prise donne un échange ; glisser une tuile dans « postes à
  placer » la retire du plan. La tuile porte le numéro, le libellé,
  l'état écrit et son icône (ACCES-4), et la miniature quand les écrans
  sont affichés, ou le motif pour lequel l'écran n'est pas visible.
- Barre d'actions : « Toute la salle (N postes) » sans sélection, ou la
  sélection ; « Identifier ». Le menu court d'une tuile : identifier,
  déplacer, retirer du plan.
- Panneau : décompte des postes par état, récapitulatif de clôture,
  adresses du relais intégré, connexions refusées, paquet
  d'installation de la salle (enregistré par le dialogue du système).
- Retours d'action et erreurs en bas à droite, avec « Annuler » quand
  l'action s'y prête.

**Interface de l'agent** (`ui/agent`) : aucune fenêtre hors séance.
L'indicateur de séance est une barre de 560 × 46 pixels, en haut de
l'écran, toujours au premier plan, sans bordure ; l'élève peut la
réduire (150 pixels) ou la déplacer, pas la fermer. « Identifier »
ouvre au centre de l'écran, pendant 6 s, le numéro de la tuile. Seul cas
de fenêtre hors séance : l'agent sans fichier de rattachement explique
qu'il ne peut pas démarrer.

**Tests** (`cargo test`, une trentaine) : certificats et refus
(`surveo-identity`), trames (`surveo-protocol`), recherche du relais
(`surveo-net`), capture de démonstration (`surveo-platform`), relais
réel avec postes en TLS et console embarquée
(`crates/surveo-relay-core/tests/relay.rs`), moteur de la console
(placement, séance, prolongation bornée, clôture et injoignables, fin
d'elle-même, retour d'« identifier »), et la séance de bout en bout
(`apps/console/tests/seance.rs`) : relais réel, moteur de la console,
agent réel qui trouve le relais par la recherche locale, s'identifie,
rejoint la séance, envoie des miniatures, suit la prolongation et
revient à son état d'origine à la clôture.

**Vérifié dans l'interface réelle** (31 octobre 2026, Fedora 44, écran X
virtuel, console et quatre agents sur le même poste, pilotés au clic) :
découverte des postes par la recherche locale ; placement au clic ;
ouverture : postes « dans la séance » et indicateur « Ce poste est relié
à la séance de M. Durand jusqu'à 10:40. » ; miniatures de démonstration
et capture X11 réelle ; prolongation avec « Annuler » ; « Identifier » et
son retour « envoyé à 3 postes » ; console relancée : séance reprise,
postes reconnectés en 3 s ; agent relancé : hors séance, puis séance
rejointe ; clôture avec un poste arrêté brutalement : « poste 03 :
injoignable, remise en état à son prochain démarrage ou à 11:05 », puis
« 3 postes remis dans leur état d'origine » à son retour.

Deux défauts trouvés ainsi et corrigés : une fenêtre Tauri non
redimensionnable reçoit sous X11 une hauteur minimale de 200 pixels
(l'indicateur est donc redimensionnable, avec un minimum explicite) ; une
variable `WAYLAND_DISPLAY` présente mais vide faisait passer la capture
par le portail de la session Wayland (une variable vide compte désormais
comme absente).

**Limites, à éprouver en salle**

- Windows : agent et console compilés, pas encore lancés. Capture
  Wayland : code du prototype `capture` (1.1), non revérifié dans
  l'agent.
- Sous Wayland, le bureau décide de la place de l'indicateur ; il n'est
  pas forcément au premier plan.
- Une seule console : elle détient la clé de l'établissement et l'état
  des salles. Le plan appartient à la salle (SALLE-2) mais vit dans
  cette console tant que la configuration n'est pas poussée par le
  relais (RECONF-4).
- Qui détient le paquet d'une salle peut y ajouter des postes ; sa
  révocation viendra avec le chapitre 6.
- Une console et des agents sur un même poste : l'agent retient
  l'adresse `127.0.0.1`, inutile ailleurs ; sans conséquence en salle.
- Les applications se compilent seulement après les interfaces
  (`ui/dist`).

## 2. Maîtrise de la classe

### 2.1 Prototype `sans-admin`

Code dans `prototypes/sans-admin/` (jetable). Un seul programme, dont
les commandes suivent la vie d'un agent :

| Commande | Rôle | SPEC.md |
|---|---|---|
| `installer` | Copie l'agent dans le dossier de l'utilisateur et l'inscrit au démarrage de session | INST-1, INST-6 |
| `filtrer bloquer\|autoriser SITES` · `filtrer tout` `[--duree S] [--pac]` | Relève le réglage de proxy d'origine dans un journal, pose le proxy local, filtre jusqu'à la fin prévue, surveille le contournement, puis rétablit le réglage et le compare à l'origine | WEB-1, WEB-2, RESTR-1, RESTR-4, MODE-3 |
| `demarrage` | Lancé à l'ouverture de session : si le journal existe, la séance précédente s'est mal terminée et le réglage d'origine est rétabli | RESTR-3, DEMAR-2 |
| `etat`, `desinstaller` | Diagnostic ; désinstallation complète | INST-5 |

**Réalisation**

- **Proxy de l'utilisateur** : sous Windows, API WinINet
  (`InternetSetOptionW`, options par connexion), qui règle le proxy du
  seul utilisateur sans droits d'administration et prévient les
  applications du changement ; sous Linux, clés `org.gnome.system.proxy`
  par `gsettings`. Le réglage d'origine est relevé en entier (type,
  serveur, exceptions, script de configuration) et restitué tel quel.
- **Journal** : écrit avant de toucher au réglage, effacé après l'avoir
  rétabli. S'il existe au démarrage, la séance précédente s'est mal
  terminée.
- **Démarrage de session** : clé `Run` de l'utilisateur sous Windows,
  fichier dans `~/.config/autostart` sous Linux.
- **Proxy local de filtrage** sur `127.0.0.1`, port choisi au hasard :
  en HTTPS, il ne voit que le nom du site (requête `CONNECT`) ; en HTTP,
  il voit la requête entière et peut répondre par une page.
- **Deux façons de poser le proxy** : proxy fixe, ou script de
  configuration automatique (PAC) servi par l'agent lui-même et qui dit
  « proxy local, sinon connexion directe ».

**Résultats** (29 octobre 2026)

| Vérification | Linux (COSMIC, Fedora 44) | Windows (compte standard) |
|---|---|---|
| Installation et démarrage de session sans élévation | ✅ | ✅ aucune fenêtre UAC |
| Réglage préexistant rétabli à l'identique | ✅ (un vrai proxy automatique préexistant) | ✅ |
| Site HTTPS bloqué | ✅ erreur propre au navigateur | ✅ erreur propre au navigateur |
| Site HTTP bloqué | ✅ page « Accès suspendu » | ✅ page « Accès suspendu » |
| Élève qui coupe le proxy | ✅ détecté en 2 s | ✅ détecté |
| Arrêt brutal, proxy fixe | Plus d'accès au web jusqu'à `demarrage`, qui rétablit l'origine | Idem ; rétabli à la reconnexion |
| Arrêt brutal, script PAC | — | ✅ le web refonctionne aussitôt (connexion directe) |
| Désinstallation | ✅ rien ne reste | ✅ |

Le blocage reste effectif en mode PAC tant que l'agent tourne : un refus
du proxy ne fait pas basculer le navigateur en connexion directe ;
seule l'absence du proxy le fait.

**Enseignements**

1. **Une page explicative n'est possible qu'en HTTP.** Pour un site en
   HTTPS, presque tous aujourd'hui, le proxy ne peut que refuser la
   connexion et le navigateur affiche sa propre erreur. Afficher notre
   page exigerait d'installer un certificat racine, ce qui est hors de
   question. L'agent, lui, sait qu'il vient de bloquer un site : c'est à
   lui d'en prévenir l'élève, par une notification (proposition de
   modification de WEB-2).
2. **Sans droits d'administration, l'élève peut arrêter l'agent.** Avec
   un proxy fixe, le poste n'a alors plus accès au web jusqu'à la
   prochaine ouverture de session, bien au-delà de la séance (contraire à
   RESTR-1). Avec le script PAC, la restriction s'arrête avec l'agent :
   le navigateur se connecte directement. Le mode PAC est donc à retenir
   pour le déploiement sans droits d'administration ; la console signale
   l'arrêt de l'agent comme une sortie de restriction (MODE-3).
3. **Antivirus** : sur le poste de test, Avast bloque l'exécutable non
   signé (il a fallu le désactiver) ; Microsoft Defender ne réagit pas.
   Avast n'est pas installé au lycée de l'utilisateur, mais le risque
   demeure ailleurs (prototype `antivirus`).

**Limites**

- Chrome sous Linux ne lit les réglages GNOME que dans un bureau qu'il
  reconnaît (GNOME, KDE…) ; ailleurs il suit les variables
  d'environnement. Le comportement des navigateurs sous COSMIC et sous
  KDE reste à vérifier un par un.
- Le réglage PAC restant après un arrêt brutal est inoffensif
  (connexion directe), mais il n'est nettoyé qu'au démarrage suivant.
- Un programme qui n'utilise pas le proxy du système (client de jeu,
  certaines applications) n'est pas filtré : le filtrage sans droits
  d'administration ne vaut que pour les navigateurs.

### 2.2 Maîtrise en séance

Deuxième tranche de la phase 4 : l'enseignant cadre l'usage des postes
pendant la séance. Elle s'appuie sur la séance minimale (1.2) et reprend
le prototype `sans-admin` (2.1). Version 0.2.0, protocole 2 : un agent
de la tranche 1 est refusé par la console avec le motif « version
incompatible ».

**Règles couvertes** : MAIT-1, MAIT-2, MAIT-3 (affichage du mode),
MAIT-4 (consignation) ; ECRAN-1 à ECRAN-4 ; WEB-1 à WEB-4 ; APP-1 à
APP-5 ; EXAM-1 à EXAM-3 ; PERSO-3 côté agent ; MODE-1 à MODE-6 (mode
sans droits d'administration) ; RESTR-1 à RESTR-4, DEMAR-2 ; SEL-3,
RETOUR-1 à RETOUR-3. Hors de la tranche : EXAM-4 (rendus, chap. 3),
analyse par l'IA et conversation en examen (chap. 4 et 5), EXAM-5 et
SEL-4 tant qu'il n'y a pas de poste personnel, journal d'audit
consultable (chap. 6 et 8), mode « déployé par l'établissement ».

**Un état déclaratif par poste** (`crates/surveo-protocol`). La console
envoie à chaque poste l'état complet de ses restrictions, `Restrict`
{pause et son message, filtrage web, applications interdites, mode
examen avec sa durée restante et ses sites}, à chaque changement et à
chaque reconnexion, après `Session` ; le poste applique la différence.
Autres messages : `ListApps` et la réponse `Apps`, `CloseApp` ; `Status`
porte en plus l'état du filtrage (éteint, en place, quitté par l'élève,
impossible avec son motif). Un site de liste couvre ses sous-domaines ;
ce que l'enseignant écrit (`https://www.Wikipedia.org/wiki`) est ramené
au nom du site (`wikipedia.org`).

**La console** (`apps/console/src/engine.rs`) garde les restrictions
avec la séance (`seance.json`), chacune en deux couches : la valeur de
toute la salle, et celle des postes d'une sélection. Une action sans
sélection fixe la valeur de la salle, qui vaut aussi pour un poste
rejoignant la séance plus tard (SEL-2) ; avec une sélection, elle ne vise
que ces postes (MAIT-2). Le mode examen est un état de la séance, avec
son heure de fin, au plus tard celle de la séance : raccourcir la séance
le raccourcit, et la console le lève à son heure.

- Chaque action donne son retour poste par poste ; un poste absent est
  « en attente, appliqué à son retour » (RETOUR-1, RETOUR-2). Le retour
  porte « Annuler », qui rétablit l'état d'avant tant qu'aucune autre
  action n'a suivi (RETOUR-3).
- Chaque restriction est consignée dans `journal.jsonl` : qui, quoi,
  quand, salle, séance, postes visés (MAIT-4).
- Les listes de sites et d'applications de l'enseignant sont dans
  `listes.json` (WEB-4, APP-4) ; une liste de sites sert aussi bien
  d'autorisation que de blocage.
- Sorties de restriction (MODE-3, MODE-5) : un poste qui annonce son
  filtrage quitté donne une alerte et un message ; un poste restreint
  qui perd la connexion pendant la séance donne une alerte, complétée
  par l'heure de son retour.

**L'agent** (`apps/agent/src/restrict.rs`) applique les restrictions et
reste autonome : il les garde pendant une coupure (RESTR-2), lève le
mode examen à sa propre échéance, et tout lève en quittant la séance.
Toutes les deux secondes, il vérifie la fin de l'examen, les
applications interdites et le réglage du proxy. Il ignore toute
restriction sur un poste personnel (PERSO-3).

- **Écran en pause** : une fenêtre plein écran par écran, au premier
  plan, qui ne se ferme pas et reprend le premier plan chaque seconde ;
  rien n'est fermé dessous (ECRAN-4). Message par défaut « Écran mis en
  pause par {enseignant}. », ou celui de l'enseignant.
- **Filtrage web** (`webfilter.rs`, repris de `sans-admin`) : proxy
  local sur `127.0.0.1`, réglage de l'utilisateur par script PAC, servi
  par l'agent (MODE-5) ; sous Linux seuls le mode et l'adresse du script
  changent (`gsettings`), sous Windows le réglage WinINet
  (`crates/surveo-platform/src/proxy.rs`). La politique change sans
  toucher au réglage. Un site bloqué en HTTPS est refusé ; en HTTP, une
  page dit pourquoi ; dans les deux cas l'élève reçoit « {site} n'est
  pas accessible pendant la séance de {enseignant}. » (WEB-2), au plus
  une fois par minute pour un même site et toutes les quatre secondes en
  tout. Pendant l'examen, sa liste d'autorisation remplace le filtrage
  du poste, qui revient ensuite.
- **Journal de remise en état** (`remise-en-etat.json`) : le réglage
  d'origine y est écrit avant d'être modifié, et effacé une fois rétabli.
  Au démarrage, avant toute connexion, un journal resté là est rejoué
  (RESTR-3, DEMAR-2). L'agent ne rétablit un réglage que s'il est encore
  le sien : un réglage changé entre-temps par quelqu'un d'autre reste tel
  quel (RESTR-4). Si l'élève remplace le réglage pendant la séance,
  l'agent l'annonce (MODE-3) ; réappliquer le filtrage le repose.
- **Applications** (`crates/surveo-platform/src/apps.rs`) : sous Linux,
  les processus de l'utilisateur, rapprochés des lanceurs `.desktop`
  (nom, exécutable, dossier de l'exécutable, Flatpak), ce qui marche sous
  Wayland comme sous X11 ; sous Windows, les processus de la session et
  leurs fenêtres, nommés par la description de l'exécutable. Les
  programmes du bureau (barre des tâches, gestionnaire de fenêtres…),
  l'agent et sa descendance ne sont jamais listés ni fermés (APP-3). Un
  nom interdit désigne l'exécutable, ou des mots entiers du nom de
  l'application : « minecraft » ferme « Minecraft Launcher ». Fermeture
  polie (`SIGTERM`, `WM_CLOSE`), puis forcée quatre secondes plus tard.
  L'élève reçoit « {application} a été fermée par {enseignant}. », une
  fois par application et par demi-minute.
- **Indicateur** : il énonce les restrictions en cours en mots simples,
  sur une seconde ligne (« Web : seuls quelques sites sont ouverts ·
  Applications interdites : Minecraft ») (MAIT-1) ; en examen, « Séance
  d'examen en cours jusqu'à {heure}. » (EXAM-1).
- **Fenêtres** (`apps/agent/src/main.rs`) : créées et fermées par un fil
  à part, dans l'ordre des demandes. Une création peut prendre plusieurs
  secondes (5 s mesurées dans l'environnement d'essai) et ne retarde ainsi
  ni le proxy, ni les réponses à la console.

**Interface de la console** (`ui/console`) :

- Barre d'actions : « Pause » (ou « Reprendre » si tous les postes visés
  sont en pause), avec messages proposés ; « Plus » : filtrage web,
  applications interdites, applications ouvertes (un seul poste
  sélectionné), mode examen, identifier (SEL-3). Menu court de la tuile :
  mettre en pause ou reprendre, applications ouvertes.
- Bandeaux des restrictions de toute la salle, chacun avec son geste
  pour la lever : mode examen, écrans en pause, filtrage web,
  applications interdites.
- Tuile : marques des restrictions en cours avec leur nom (pause,
  filtrage, applications interdites, filtrage quitté), fond hachuré en
  pause (AFF-2) ; mode de déploiement dans l'infobulle.
- Panneau : mode de déploiement des postes et ce qu'il garantit (MODE-2,
  MODE-6) ; sorties de restriction.
- Échap ferme d'abord la fenêtre d'action ouverte.

**Interface de l'agent** (`ui/agent`) : écran en pause, notification
discrète sous l'indicateur (huit secondes), indicateur sur deux lignes.

**Tests** (`cargo test`, 57) : protocole (politique web, noms de
sites, compatibilité des messages) ; moteur de la console (couches salle
et sélection, poste qui rejoint, retour et annulation, consignation,
postes absents servis à leur retour, examen borné par la séance et levé
à son heure, sorties de restriction, listes) ; agent (journal et
rétablissement, sortie de restriction et réapplication, réglage changé
par ailleurs, arrêt brutal puis démarrage, examen qui remplace puis rend
le filtrage, frein des notifications ; proxy local) ; plateforme
(lanceurs, regroupement des processus, programme réel trouvé puis
fermé) ; et la séance de bout en bout, étendue à la pause, au filtrage
(site refusé, élève prévenu, sortie signalée, origine rétablie), aux
applications interdites (programme réel fermé, élève prévenu), à la
liste des applications ouvertes et au mode examen. Le réglage du proxy y
est simulé (`MemoryProxy`).

**Vérifié dans l'interface réelle** (31 octobre 2026, Fedora 44, écran X
virtuel, console et trois agents sur le même poste ; chaque agent dans
son propre bus D-Bus avec ses réglages `gsettings` à part, le premier
avec un proxy manuel « du service informatique ») : pause d'un poste
avec message, reprise et « Annuler » ; liste d'autorisation sur toute la
salle : script PAC posé par chaque agent, Wikipédia accessible par le
proxy, YouTube refusé, page HTTP explicative, notification à l'élève ;
arrêt brutal d'un agent puis relance : origine rétablie avant la
connexion, puis filtrage reposé ; faux jeu (`minecraft`) fermé dès son
lancement et à chaque relance ; liste des applications ouvertes ; mode
examen : bandeau, indicateur « Séance d'examen en cours », Wikipédia
bloqué ; fin d'elle-même de l'examen puis de la séance, proxy d'origine
de chaque poste rétabli à l'identique ; clôture par-dessus une pause :
écrans de pause et indicateurs retirés.

Trois défauts trouvés ainsi et corrigés : la fenêtre « Applications
ouvertes » redemandait la liste à chaque mise à jour de l'état, en
boucle, ce qui saturait la file du poste au relais et lui a fait perdre
une commande ; le proxy attendait l'affichage de la notification avant
de répondre (5 s) ; une alerte « connexion perdue » restait sans suite
après le retour du poste.

**Limites, à éprouver en salle**

- Windows : proxy WinINet, applications (fenêtres, description de
  l'exécutable, fermeture) et écran en pause compilés, pas encore lancés.
- Sans droits d'administration, la maîtrise est coopérative : un élève
  peut quitter l'écran en pause (Alt+Tab) ; il reprend le premier plan
  chaque seconde. Sous Wayland, le bureau peut ne pas le garder au
  premier plan.
- Linux : seul le réglage GNOME du proxy est posé ; sous KDE (réglage
  `kioslaverc`) et COSMIC, le comportement des navigateurs reste à
  vérifier. Un navigateur réglé sur son propre proxy, ou un programme
  qui ignore celui du système, n'est pas filtré (MODE-6).
- Un agent arrêté proprement (fin de session) ne rétablit pas le
  réglage : le script PAC devenu injoignable laisse passer le web, et le
  démarrage suivant rétablit l'origine.
- En mode PAC, des requêtes de fond des navigateurs vers des sites
  bloqués peuvent donner des notifications sans geste de l'élève : à
  observer en salle.
- Une application interdite est reconnue par son nom ; un jeu lancé
  sous un nom d'exécutable générique (`javaw`) n'est fermé que si
  l'enseignant nomme cet exécutable.
- Une commande perdue par le relais (file d'un poste saturée) n'est pas
  renvoyée avant la reconnexion du poste ; l'état déclaratif rend un
  renvoi périodique possible si le cas se présente.
- Plusieurs écrans : une fenêtre de pause par écran, non vérifié avec
  plusieurs écrans réels.

## 3. Pédagogie

### 3.1 Pédagogie en séance

Troisième tranche de la phase 4 : l'enseignant prépare ses TP et ses
sondages, les conduit en séance, distribue des fichiers et ramasse les
rendus. Elle s'appuie sur les tranches 1 et 2. Version 0.3.0,
protocole 3 : un agent 0.2 est refusé par la console avec le motif
« version incompatible ».

**Règles couvertes** : MODELE-1 à MODELE-4, MODELE-6 ; TP-1 à TP-9 ;
RENDU-1 à RENDU-8 ; DISTRIB-1 à DISTRIB-8 ; SONDAGE-1 à SONDAGE-6 ;
EXAM-4 ; ESPACE-1 (Préparer : modèles de TP et sondages), ESPACE-2,
ESPACE-4 ; SEL-3 ; PANNEAU-1, PANNEAU-3 ; AGENT-3, AGENT-4 ; RETOUR-1,
RETOUR-2, RETOUR-4 (remplacer un TP). Hors de la tranche : tout ce qui
passe par l'IA (MODELE-5, action « aide de l'IA », SONDAGE-7), les noms
d'élèves (ELEVE : rendus et réponses sont rangés par poste, ELEVE-3),
le poste personnel (DISTRIB-9 et PEDA-1 sont prévus dans l'agent mais
non éprouvés), le repli du panneau (PANNEAU-2), la vue liste et ses
critères (« ceux qui n'ont pas rendu »), la conservation (chap. 8).

**Transferts de fichiers** (`crates/surveo-protocol`,
`crates/surveo-relay-core`). Un fichier est proposé (`FileOffer` :
identifiant de l'envoi, nom, taille, empreinte SHA-256), puis tiré par
son destinataire, un morceau de 256 Kio à la fois : `Fetch` demande un
morceau, une trame binaire `Chunk` le porte. Le relais fait passer ces
trames par la file des messages de contrôle, dans l'ordre et dans les
deux sens (seules les miniatures gardent la file « la plus récente ») ;
une console embarquée envoie les siennes par `LocalSender::send_binary`.
Un morceau sans réponse au bout de 10 s est redemandé là où le transfert
en était : les files bornées du relais ne débordent pas, et un transfert
interrompu reprend. L'empreinte est vérifiée à l'arrivée. Tailles
maximales : 50 Mo pour un rendu (RENDU-7), 200 Mo pour un fichier
distribué ou joint.

**Messages** (protocole 3) :

| Sens | Messages |
|---|---|
| Console → postes | `Tp` (TP du poste ou aucun, avec ses étapes et ses fichiers joints), `Files` (tous les fichiers distribués au poste), `Poll` (sondage, la réponse connue du poste, les résultats montrés), `Fetch` (morceau d'un rendu), `Receipt` (rendu rangé, ou refusé avec son motif) |
| Poste → consoles | `Status` porte en plus l'avancement dans le TP (étapes cochées, retard) et les fichiers reçus ; `Fetch` (morceau d'un fichier distribué), `Submit` (rendu à tirer), `Answer` (réponse au sondage) |
| Binaire, dans les deux sens | `Chunk` : identifiant du fichier, position, données |

Comme les restrictions, `Tp`, `Files` et `Poll` sont des états complets,
renvoyés à chaque changement et à chaque reconnexion après `Session` :
un poste qui rejoint la séance plus tard ou revient d'une coupure se
remet à jour seul (TP-2, DISTRIB-4). La console garde donc l'état de la
séance, le relais rien de plus (question ouverte « Report pendant une
déconnexion » : tranchée ainsi pour ces messages).

**La bibliothèque** (`apps/console/src/library.rs`) contient les modèles
de TP (titre, étapes, rendus attendus, fichiers joints) et les sondages,
dans `bibliotheque.json`. Les fichiers joints et distribués sont copiés
dans le magasin de la console (`fichiers/`), nommés par leur empreinte ;
le magasin est vidé de ce que plus rien ne désigne au démarrage et à la
clôture. Un modèle ou un sondage s'exporte en un fichier `.surveo` (JSON,
fichiers joints en base 64), qu'une autre console importe à l'identique,
empreintes vérifiées (MODELE-4). Un sondage lancé en séance y est
enregistré (SONDAGE-6).

**La console** (`apps/console/src/engine/peda.rs`) :

- TP : le lancer en fait une copie (TP-1), rangée avec la séance ; le TP
  de chaque poste suit les deux couches de la tranche 2 (toute la salle,
  sélection), si bien qu'un poste qui arrive reçoit le TP lancé sur toute
  la séance (TP-2). Remplacer le TP d'un poste se confirme dans
  l'interface (TP-4) ; l'arrêter (TP-9) lui envoie « aucun TP ». La
  synthèse compte les postes à chaque étape, ceux qui ont tout fini, les
  retards et les rendus complets (TP-8) ; un poste qui n'a encore rien
  annoncé est compté à la première étape.
- Distribution : chaque fichier distribué l'est à toute la séance ou à
  des postes ; chaque poste reçoit la liste complète des siens et tire
  ceux qui lui manquent. La console sait, par l'état des postes, lesquels
  sont reçus (DISTRIB-4).
- Rendus : un rendu proposé est tiré dans `reception/`, puis rangé dans
  `<dossier des rendus>/<séance>/<TP>/<poste>` (RENDU-6) : dossier de la
  séance « 2026-10-31 14h05 B204 », du TP par son titre, du poste
  « poste 07 (PC-B204-07) » ; il est indexé dans `rendus.json`, puis
  accusé (`Receipt`). Un même nom rendu de nouveau : la version
  précédente est renommée « compte-rendu (14h20m05).odt », la plus récente
  garde le nom (RENDU-5) ; le compteur de la tuile compte les noms. Un
  rendu proposé de nouveau (accusé perdu) est accusé sans être rangé deux
  fois. L'index garde les séances closes : un rendu déposé pendant la
  séance et arrivé après est rangé quand même (RENDU-3, RENDU-8). Le
  dossier des rendus est `Documents/Surveo/Rendus` (ou `DOSSIER/rendus`
  avec `--dossier`) ; la console l'ouvre pour la séance ou pour un poste.
- Sondage : un à la fois, sur toute la séance ou une sélection, avec une
  durée limite facultative à laquelle la console le clôt (SONDAGE-4). Les
  réponses, changeables tant qu'il est ouvert, vivent avec la séance et
  disparaissent à sa clôture (CONSERV-1). Montrés aux élèves, les
  résultats ne sont que des comptes par réponse (SONDAGE-5) ; le détail
  par poste reste dans la console.

**L'agent** (`apps/agent/src/peda.rs`) garde dans `peda.json` son TP
(étapes cochées, début de l'étape en cours), ses rendus en attente et la
liste des fichiers distribués qu'il a écrits :

- TP : l'étape en cours est la première non cochée ; l'élève coche et
  décoche (TP-6). Le même TP renvoyé après une coupure ou un redémarrage
  garde ses étapes (TP-3). L'étape qui dépasse sa durée met le poste « en
  retard » sans rien bloquer, et déclenche une fois son action : ouvrir
  le panneau élève sur ses consignes (TP-7, MODELE-2).
- Fichiers reçus : rangés dans `Documents/Surveo/Séance 2026-10-31
  14h05` de l'élève, sous un nom libre (« cours (2).txt ») : rien n'est
  écrasé (DISTRIB-2). Chaque chemin est noté avant l'écriture ; en fin de
  séance, ces fichiers, et eux seuls, sont retirés d'un poste de salle,
  puis le dossier s'il est vide (DISTRIB-6, DISTRIB-7) ; un fichier
  distribué modifié par l'élève est retiré lui aussi (SPEC.md, 3.9). Au démarrage, ceux d'une séance dont l'échéance est
  passée sont retirés ; ceux d'une séance encore en cours attendent de
  savoir si le poste la rejoint, pour ne pas faire perdre son travail à
  un élève dont le poste a redémarré. L'élève est prévenu de la
  distribution (DISTRIB-3) et, cinq minutes avant la fin, du retrait
  (DISTRIB-8).
- Rendus : au dépôt, l'agent copie le fichier dans son dossier (le
  fichier de l'élève reste en place, et ce qui est rendu est figé à
  l'heure du dépôt), puis le propose ; sans connexion il attend, et part
  au retour de la connexion, même après la séance (RENDU-3). Un rendu
  sans nouvelles de la console est proposé de nouveau toutes les 30 s.
  La zone de dépôt n'existe qu'avec un TP en cours dans la séance
  (RENDU-8).
- Sondage : la fenêtre du sondage s'impose au premier plan tant que
  l'élève n'a ni répondu ni choisi « Répondre plus tard » (AGENT-4) ; une
  réponse donnée pendant une coupure repart quand la console renvoie le
  sondage sans elle.

**Interface de la console** (`ui/console`) :

- Espaces Préparer et Séance dans la barre du haut (ESPACE-1) ; dans
  Préparer, un bandeau rappelle la séance en cours et y ramène
  (ESPACE-4).
- Préparer : modèles de TP (fiches, détail des étapes, modifier,
  dupliquer, exporter, supprimer après confirmation, importer) et leur
  éditeur (étapes ordonnées, consignes en Markdown avec aperçu, durée,
  action en cas de dépassement, fichiers joints choisis par le dialogue
  du système) ; sondages (question, réponses, choix unique ou multiple).
- Barre d'actions de la séance : « Fichier », « TP », « Sondage » à côté
  de « Pause » (SEL-3). Un fichier glissé du bureau sur le plan est
  distribué aux postes visés, sur une tuile à ce poste (DISTRIB-1).
- Panneau : synthèse de chaque TP (barres par étape, retards et
  « Sélectionner les postes en retard », rendus complets, dossier des
  rendus, « Arrêter le TP ») ; fichiers distribués et leur réception ;
  sondage en direct (barres, taux de réponse, détail par poste, clore,
  montrer aux élèves). Le récapitulatif de clôture ouvre les rendus.
- Tuile : « Étape 2/3 · Rendus 1/2 », marques « en retard », « fichiers
  en cours de réception », « a répondu au sondage » ; menu « Ouvrir ses
  rendus ».
- Les consignes en Markdown sont rendues par `ui/shared/markdown.ts` :
  tout est échappé avant la mise en forme, et un lien s'ouvre dans le
  navigateur du poste, jamais dans la fenêtre Surveo.

**Interface de l'agent** (`ui/agent`) : le bouton « Surveo » de
l'indicateur ouvre le panneau élève (AGENT-3), fenêtre ordinaire à
droite de l'écran, qui s'ouvre aussi seule au lancement d'un TP, sans
prendre le premier plan. Il montre le TP (étapes à cocher, consignes de
l'étape en cours, temps indicatif restant ou dépassé, fichiers joints à
ouvrir), la zone de dépôt (glisser un fichier sur la fenêtre, ou
« Choisir un fichier… ») avec l'état de chaque rendu (« en attente
d'envoi : la connexion avec l'enseignant est interrompue », « reçu par
l'enseignant à 14:32 »), les fichiers reçus et leur dossier, le sondage
et ses résultats, et à la fin le message de TP-9. Hors séance, il
disparaît, sauf pour annoncer la fin du TP. Les fichiers s'ouvrent par
l'application habituelle du poste (`surveo_platform::files::open_path`).

**Tests** (`cargo test`, 91 dont 44 exports de types) : transferts
(trames de morceaux, relais réel dans les deux sens, ordre conservé) ;
fichiers (noms sûrs, jamais d'écrasement, empreintes) ; bibliothèque
(modèle vérifié, dupliqué, exporté puis importé à l'identique sur une
autre console, export abîmé refusé, ménage du magasin) ; console (TP
lancé sur la salle puis reçu par un poste qui arrive, modèle modifié
sans effet sur le TP lancé, remplacement et arrêt, synthèse ; rendus
tirés en plusieurs morceaux, rangés par séance, TP et poste, versions,
accusé perdu, trop volumineux, rendu arrivé après la clôture, reprise
d'un transfert ; distribution ciblée puis à toute la séance, morceau
servi, fichier non destiné ; sondage en direct, résultats sans nom,
clôture à la durée limite) ; agent (étapes, retard et son action une
seule fois, TP retrouvé après redémarrage, arrêt ; fichiers sans
écrasement, avertissement de fin, retrait en fin de séance et au
démarrage suivant d'un poste éteint ; rendu pendant une coupure puis
après la fin de la séance, accusé seulement à la réception, zone de
dépôt fermée ; refus d'un rendu trop gros ; sondage au premier plan,
réponse renvoyée après une coupure) ; et la séance de bout en bout,
étendue au TP avec un fichier joint de 600 Ko, à l'étape cochée vue de la
console, à la distribution, au rendu, au sondage et au retrait des
fichiers à la clôture.

**Vérifié dans l'interface réelle** (31 octobre 2026, Fedora 44, écran X
virtuel, console et deux agents sur le même poste, pilotés au clic) :
modèle créé dans Préparer (consignes en Markdown et aperçu, durée d'une
minute, consignes au dépassement, fichier joint de 700 Ko choisi par le
dialogue) ; séance ouverte, TP lancé sur toute la salle : panneau élève
ouvert seul, fichier joint reçu, tuiles « Étape 1/2 · Rendus 0/1 » ;
étape cochée chez l'élève et vue aussitôt dans la console ; retard après
une minute (tuile, synthèse, « temps indicatif dépassé » chez l'élève) ;
rendu par « Choisir un fichier… », rangé dans « 2026-10-31 16h54 B204 /
Adressage IPv4 / poste 01 (fixeolive) », accusé chez l'élève ;
distribution par le dialogue, reçue sur 2/2, « cours (2).txt » à côté du
« cours.txt » de l'élève ; sondage au premier plan, une réponse et un
« plus tard », résultats en direct, détail par poste, résultats montrés
sans nom ; « Ouvrir » un fichier reçu et « Ouvrir les rendus » ; clôture :
fichiers distribués retirés, fichier de l'élève intact, dossier vide
supprimé, « Séance terminée. Vos rendus envoyés ont bien été reçus. » ;
console arrêtée brutalement pendant un TP, rendus déposés pendant la
coupure (« en attente d'envoi »), console relancée : séance reprise,
rendus arrivés seuls, deux versions gardées, accusés à ce moment-là ;
agent redémarré en plein TP : étapes et rendus retrouvés, panneau non
rouvert.

**Éprouvé sous Windows** par l'utilisateur (31 octobre 2026) : la
version 0.3.0 fonctionne.

Cinq défauts trouvés ainsi et corrigés : les barres des résultats du
sondage avaient une largeur nulle dans le panneau ; une réponse donnée
dans la fenêtre du sondage n'était pas cochée dans le panneau élève ; la
liste « Si dépassement » coupait « Afficher les consignes » ; le pied des
tuiles coupait « Étape 1/2 » en deux ; la confirmation de clôture ne disait
pas que les fichiers distribués seraient retirés.

**Limites, à éprouver en salle**

- Le glisser-déposer d'un fichier du bureau sur le plan de la console
  n'a pas été éprouvé (pas de gestionnaire de fichiers dans l'écran
  virtuel) ; le fichier est lu en mémoire par l'interface, 200 Mo au
  plus.
- Débit : un morceau à la fois par transfert. Sur le réseau local c'est
  largement suffisant ; à mesurer avec un relais hébergé (latence), où
  une fenêtre de quelques morceaux pourrait être utile. La console sert
  tous les postes d'une distribution : 30 postes × 50 Mo, c'est 1,5 Go à
  envoyer depuis le poste de l'enseignant.
- Un élève peut retirer lui-même un fichier distribué ou le déplacer :
  Surveo ne retire que les chemins qu'il a écrits. Un fichier distribué
  modifié est retiré en fin de séance : l'élève en est prévenu cinq
  minutes avant.
- Sous Wayland, comme l'indicateur, le panneau et le sondage sont placés
  par le bureau.
- Dans l'environnement d'essai (bus D-Bus privé), l'agent met une
  vingtaine de secondes à afficher ses fenêtres, le temps que le portail
  du bureau, injoignable, ne réponde pas ; à vérifier sur une vraie
  session.
- Pas de conservation limitée des rendus (CONSERV-2, chap. 8) : ils
  restent tant que l'enseignant ne les supprime pas.

## 5. IA

### 5.1 Prototype `classifieur`

Code dans `prototypes/classifieur/` (jetable). Le modèle étudié est le
classifieur de harcèlement et de détresse de la version précédente :
CamemBERT de base affiné sur 4 classes (neutre, toxique, détresse,
harcèlement), 443 Mo en float32.

| Élément | Rôle |
|---|---|
| `export/export.py` | Convertit le modèle en ONNX, fusionne les couches d'attention, quantifie les poids en INT8 ; produit aussi une variante au format ORT et une variante limitée aux opérateurs ONNX standard |
| `export/evaluate.py` | Mesure la qualité de chaque conversion sur la partie de validation du corpus d'entraînement (5 170 phrases), reconstituée à l'identique ; écrit `models/eval.jsonl` pour le banc Rust |
| `classifieur` (Rust) | Banc : découpage en jetons, inférence par ONNX Runtime ou par tract (`--backend`), temps par phrase, mémoire, accord avec PyTorch ; ou classement des phrases passées en argument |
| `dist.sh`, `mesurer.bat` | Assemblage des distributions Linux et Windows ; série de mesures à lancer sur un poste Windows |

Les modèles, le moteur téléchargé et les distributions ne sont pas
versionnés : `eval.jsonl` reprend des phrases du corpus.

**Réalisation**

- **Export** : l'exportateur classique de PyTorch (`dynamo=False`), avec
  l'attention « eager ». Avec l'exportateur récent, le graphe d'attention
  prend une forme que l'optimiseur d'ONNX Runtime ne sait pas fusionner.
  Une fois fusionné, le graphe compte 12 opérateurs `Attention`, 24
  `SkipLayerNormalization` et 12 `BiasGelu`.
- **Quantification** dynamique INT8, par canal, de toutes les
  multiplications de matrices et de la table des plongements : 111,5 Mo.
  Les poids sont réduits à 7 bits (`reduce_range`), pour avoir
  des résultats justes sur un processeur sans VNNI (enseignement 3).
- **Découpage en jetons** : crate `tokenizers`, qui lit le
  `tokenizer.json` de l'entraînement. Le remplissage à 128 jetons qu'il
  contient doit être désactivé : il triplerait le coût d'une phrase
  seule. Les expressions régulières sont en Rust pur (`fancy-regex`) :
  aucun code C à compiler pour Windows. Le découpage est identique à
  celui de Python : le banc Rust et `evaluate.py` font les mêmes
  prédictions.
- **ONNX Runtime** : crate `ort` 2.0.0-rc.15 en chargement dynamique. La
  bibliothèque officielle 1.31.1 est livrée à côté de l'exécutable
  (`onnxruntime.dll`, 17,3 Mo, signée par Microsoft ;
  `libonnxruntime.so`, 29,3 Mo). Le même code sert sous Linux et sous
  Windows, et la compilation croisée par mingw n'a pas à lier ONNX
  Runtime.
- **tract**, moteur écrit en Rust, pour comparaison (fonctionnalité
  Cargo `tract`) : il ne connaît pas les opérateurs fusionnés d'ONNX
  Runtime et lit une variante non fusionnée (`model-int8-std.onnx`), avec
  un lot fixé à 1.

**Qualité** : partie de validation, 5 170 phrases.

| Modèle | Taille | Exactitude | F1 macro | Rappel détresse | Rappel harcèlement | Rappel toxique | Accord avec PyTorch |
|---|---|---|---|---|---|---|---|
| PyTorch, float32 | 443 Mo | 93,58 % | 95,50 % | 99,63 % | 98,65 % | 91,08 % | — |
| ONNX, float32 | 443 Mo | 93,58 % | 95,50 % | 99,63 % | 98,65 % | 91,08 % | 100 % |
| ONNX, INT8 sur 7 bits (retenu) | 111,5 Mo | 93,17 % | 95,20 % | 98,90 % | 98,50 % | 89,88 % | 98,53 % |
| ONNX, INT8 sur 8 bits, processeur avec VNNI | 111,5 Mo | 92,92 % | 95,00 % | 98,90 % | 98,05 % | 89,22 % | 98,16 % |
| ONNX, INT8 sur 8 bits, processeur sans VNNI | 111,5 Mo | 43,09 % | — | 0 % | 16,17 % | 0 % | 43,35 % |
| ONNX, poids non signés (UINT8) | 111,6 Mo | 93,33 % | 95,32 % | 98,90 % | 98,50 % | 89,66 % | 98,30 % |

**Performances** (30 octobre 2026) : Linux, Intel i9-13900K, un fil
d'exécution sauf mention, phrases de 18,4 jetons en moyenne. Le cœur
« E » (4,3 GHz, plus simple) sert d'approximation d'un poste modeste.
Les lignes INT8 ont été mesurées avec les poids sur 8 bits ; ceux sur
7 bits passent par les mêmes calculs et donnent les mêmes temps (10,0 ms
en médiane, 228 Mo). Les poids non signés coûtent 17 ms en médiane.

| Configuration | Médiane | p95 | Maximum | Phrase de 128 jetons | Mémoire (pic) | Chargement |
|---|---|---|---|---|---|---|
| ONNX Runtime, INT8, cœur P | 10,0 ms | 17,2 ms | 58,6 ms | 57 ms | 225 Mo | 130 ms |
| ONNX Runtime, INT8, 4 cœurs P | 4,8 ms | 8,3 ms | 26,7 ms | — | 225 Mo | 139 ms |
| ONNX Runtime, INT8, cœur E | 29,3 ms | 57,4 ms | 212 ms | 211 ms | 224 Mo | 225 ms |
| ONNX Runtime, INT8 au format ORT sans pré-empaquetage, cœur P | 13,8 ms | 21,3 ms | 58 ms | — | 181 Mo | 73 ms |
| ONNX Runtime, float32, cœur P | 28,5 ms | 47,4 ms | 138 ms | — | 662 Mo | 463 ms |
| tract, INT8, cœur P (300 phrases) | 169 ms | 205 ms | 403 ms | — | 263 Mo | 425 ms |

**Sur disque**, avec ONNX Runtime : 137,5 Mo sous Windows (modèle
111,5, moteur 17,3, vocabulaire 2,4, exécutable 6,3) et 147,6 Mo sous
Linux (moteur 29,3, exécutable 4,4). tract porterait l'exécutable à
30 Mo.

**Poste Windows** (30 octobre 2026) : AMD Ryzen 7 5800XT (Zen 3,
8 cœurs, AVX2 sans VNNI), 16 Go, Windows 11, compte standard. Le
runtime Visual C++ est présent dans `System32`.

| Configuration | Médiane | p95 | Maximum | Mémoire (pic) | Chargement | Exactitude |
|---|---|---|---|---|---|---|
| ONNX Runtime, 7 bits, 1 fil | 14,9 ms | 28,3 ms | 103 ms | 188 Mo | 193 ms | 93,17 % ✅ |
| ONNX Runtime, 7 bits, 2 fils (1 000 phrases) | 9,5 ms | 22,6 ms | 72,9 ms | 188 Mo | 197 ms | 93,60 % ✅ |
| ONNX Runtime, poids non signés, 1 fil (1 000 phrases) | 17,5 ms | 34,6 ms | 118 ms | 188 Mo | 195 ms | 93,40 % ✅ |
| ONNX Runtime, 8 bits, 1 fil | 14,8 ms | 28,1 ms | 101 ms | 188 Mo | 194 ms | 43,09 % ❌ |
| ONNX Runtime, 8 bits, 2 fils (1 000 phrases) | 8,3 ms | 15,1 ms | 48,7 ms | 188 Mo | 193 ms | 45,00 % ❌ |
| ONNX Runtime, 8 bits, format ORT sans pré-empaquetage (1 000 phrases) | 20,6 ms | 34,5 ms | 103 ms | 175 Mo | 116 ms | 45,00 % ❌ |
| tract, 8 bits (100 phrases) | 2 086 ms | 3 360 ms | 3 995 ms | 252 Mo | 849 ms | 92,00 % ✅ |

Le défaut se reproduit sous Linux en émulant un processeur Haswell
(AVX2 sans VNNI) par `qemu-x86_64-static -cpu Haswell` : 40 %
d'exactitude sur 100 phrases avec les poids sur 8 bits ; 93 % et 100 %
d'accord avec PyTorch avec les poids sur 7 bits ; 94 % et 99 % avec les
poids non signés. Sur le poste Windows, les poids sur 7 bits donnent
exactement les mêmes prédictions que sur l'i9 (même exactitude, même
accord, mêmes rappels).

**Enseignements**

1. **Les critères sont tenus sous Linux et sous Windows**, sans
   recourir au plan B (modèle distillé) : moins de 150 Mo sur disque,
   190 à 230 Mo de mémoire, 10 à 15 ms par phrase (29 ms sur un cœur E).
2. **La quantification coûte 0,4 point d'exactitude.** Le rappel des
   classes sensibles reste au-dessus de 98 % ; c'est la classe
   « toxique » qui perd le plus (−1,2 point). `evaluate.py` sert de
   contrôle à chaque nouvelle version du modèle.
3. **Un modèle quantifié dépend du processeur.** Sans les instructions
   VNNI (AMD jusqu'à Zen 3, Intel avant 2021 environ), ONNX Runtime
   multiplie les octets par `vpmaddubsw`, qui sature sur 16 bits : avec
   des poids sur 8 bits, le modèle ne reconnaît plus ni la détresse ni
   la toxicité, sans aucune erreur visible. Des poids sur 7 bits évitent
   la saturation, sans perte de qualité ni de vitesse ; les poids non
   signés l'évitent aussi, mais sont plus lents. Toute nouvelle version d'un
   modèle doit donc être contrôlée sur un processeur sans VNNI ;
   l'émulation par QEMU suffit.
4. **ONNX Runtime plutôt que tract** : tract est 17 fois plus lent sur
   l'i9 et 140 fois plus lent sur le Ryzen (2 s par phrase), pour la même
   mémoire et la même qualité. On garde ONNX Runtime, au prix d'une
   bibliothèque à livrer. Sous Windows, celle-ci dépend du runtime
   Visual C++ (`VCRUNTIME140.dll`, `MSVCP140.dll`), présent sur le poste
   de test mais pas garanti ailleurs ; l'installer pour tout le poste
   demande des droits d'administration. Microsoft autorise à livrer ces
   DLL à côté de l'exécutable : c'est à prévoir dans le paquet de
   l'agent.
5. **Le temps de calcul n'est pas la vraie contrainte**, la mémoire
   l'est. L'analyse se fait en tâche de fond, sur des phrases terminées ;
   à une phrase toutes les quelques secondes, elle occupe moins de 1 %
   d'un cœur. En revanche, le modèle chargé occupe 220 Mo tant que le
   module est actif (190 Mo sous Windows). Il ne doit donc être chargé que si l'établissement
   a activé le module (SENS-2) : par défaut, il ne coûte rien. Le format
   ORT sans pré-empaquetage ramène la mémoire à 181 Mo, pour 40 % de
   temps en plus.
6. **Qualité du modèle** (hors de la question posée) : il confond
   parfois « toxique » et « harcèlement » ; par exemple, une insulte
   visant un logiciel est classée harcèlement. Ce sera à prendre en
   compte en fixant les niveaux et les motifs d'alerte (SPEC.md, 5.10).

**Limites**

- Le Ryzen du poste Windows et le cœur E de l'i9 restent des processeurs
  récents ; un vrai poste de salle ancien reste à mesurer.
- Les phrases de validation viennent du corpus d'entraînement, pas de
  textes d'élèves frappés en classe.
- Une construction réduite d'ONNX Runtime, limitée aux opérateurs du
  modèle (`required_operators.config`, produit lors de la conversion au
  format ORT), réduirait la bibliothèque ; non essayée.

## 6. Rôles et confiance

### 6.1 Authentification mutuelle par la clé d'établissement

Éprouvée par le prototype `relais` (7.1), réalisée dans
`crates/surveo-identity` (certificats) et `crates/surveo-net`
(`handshake.rs`, échange à la connexion).

- **Clé d'établissement** : une paire de clés Ed25519 créée localement
  par la console au premier lancement (CONF-1), avec un identifiant tiré
  du nom (`lycee-jean-moulin-3f9a0c`). Sa partie publique (identifiant,
  nom, clé publique) est livrée avec chaque paquet de rattachement
  (INST-3) ; sa partie privée reste dans la console
  (`etablissement.json`, lisible par son seul propriétaire sous Linux).
- **Certificat de membre** : chaque relais, console ou agent a sa propre
  paire de clés Ed25519 et un certificat {version, établissement,
  membre, rôle, salle, clé publique}. La signature porte sur ces champs
  sérialisés en JSON dans un ordre fixe, précédés d'une étiquette
  (`surveo-cert-v1`). La console émet, à la création de l'établissement,
  son propre certificat et celui de son relais intégré.
- **Paquet de rattachement** (INST-2, INST-3) : la console émet pour une
  salle une clé d'installation, de rôle « installation », limitée à
  cette salle et signée par l'établissement. Au premier démarrage,
  l'agent crée sa paire de clés et signe lui-même son certificat de
  poste avec cette clé. Il présente ensuite les deux certificats : le
  relais vérifie que l'installation est signée par l'établissement, que
  le poste l'est par l'installation, et que leurs salles concordent. Le
  poste rejoint ainsi son établissement et sa salle sans saisie et sans
  réseau ; la salle d'un poste vient de son certificat, jamais de ce
  qu'il déclare. Une clé d'installation ne peut ni se connecter, ni
  certifier autre chose qu'un poste.
- **Échange à la connexion**, après la négociation TLS :
  1. le relais envoie la version du protocole, son certificat et un défi
     aléatoire ;
  2. le client vérifie le certificat du relais avec la clé publique de
     l'établissement, puis renvoie sa version, son certificat, son propre
     défi, sa signature du défi du relais et, pour un poste, sa
     description (libellé, système, version, type, mode) ;
  3. le relais vérifie la version, le certificat et la signature du
     client, puis signe le défi du client ;
  4. le client vérifie cette signature.
- **Liaison à la session TLS** : chaque signature porte aussi sur une
  valeur exportée de la session TLS (export de clé, étiquette
  `EXPORTER-surveo-v1`). Un intermédiaire qui ouvrirait deux sessions
  TLS distinctes ne pourrait pas faire accepter une signature d'une
  session sur l'autre. Le certificat TLS du relais reste auto-signé,
  engendré à chaque démarrage : la confiance vient de la clé
  d'établissement, pas d'une autorité de certification.
- **Refus** : un membre d'un autre établissement, un certificat non
  signé, un poste qui aurait changé de salle, une autre version du
  protocole entraînent un refus explicite, journalisé par le relais et
  signalé à la console (« Connexions refusées », CONF-3).

Pas encore réalisé : le retrait d'une console (PAIR-5), le code
d'appairage (PAIR-1), la révocation d'un paquet de rattachement, la
sauvegarde de la clé d'établissement.

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

### 7.2 Prototype `tauri`

Code dans `prototypes/tauri/` (jetable), deux applications Tauri 2 :

| Application | Rôle |
|---|---|
| `console` | Console de l'enseignant. Elle **embarque le relais** du prototype 1 (RELAIS-1, relais dans la classe) et affiche le plan de salle avec les miniatures reçues. Sélection de postes, message avec retour poste par poste (RETOUR-1), identification des postes (SALLE-4), file des demandes d'aide (AIDE-1 à AIDE-4) |
| `eleve` | Panneau élève dans une vue web, côté agent (AGENT-3). Il se connecte au relais comme un poste, trouve le relais par son annonce, se reconnecte seul, envoie une miniature par seconde, affiche les messages et l'identification, transmet les demandes d'aide |

Pour cela, le cœur du relais du prototype 1 est passé dans sa
bibliothèque (`relay_core`), partagé par le relais autonome et la
console ; les commandes peuvent viser certains postes et porter un
texte ; un message `Help` va de l'agent aux consoles ; `agent-sim
--jpeg` envoie de vraies miniatures. Le prototype 1 passe toujours ses
critères après ce changement.

Les deux applications sont compilées pour Windows depuis Fedora (cible
`x86_64-pc-windows-gnu`) ; chacune pèse 17 Mo et doit être accompagnée
de `WebView2Loader.dll` (160 Ko), le lien avec le moteur WebView2 du
poste.

**Résultats** (29 octobre 2026)

| Vérification | Linux (COSMIC) | Windows (compte standard) |
|---|---|---|
| Console et panneau élève s'ouvrent | ✅ | ✅ WebView2 présent, aucun droit d'administration demandé |
| Postes trouvant la console par son annonce | ✅ 30 agents + panneau élève | ✅ 30 agents + panneau élève |
| Demande d'aide, « Traité », message avec retour par poste, identification | ✅ | ✅ |
| Antivirus | — | ⚠ Defender (SmartScreen) juge les deux exécutables non sûrs |
| Pare-feu | — | ⚠ autorisation demandée pour les deux applications, avec un mot de passe |

**Réseau réel** : la console tournant sur le poste Windows
(192.168.1.176), 30 postes simulés lancés depuis le PC Linux l'ont
trouvée par son annonce et s'y sont authentifiés en 52 ms. Avec 61
postes (30 simulés sous Windows, le panneau élève, 30 depuis Linux),
l'aller-retour d'une commande vaut 1,5 ms en médiane et 2,0 ms au 95ᵉ
centile, sans acquittement manquant. La console, relais compris,
consomme 0,1 à 0,5 % de CPU sous Windows. Cette mesure lève la
principale limite du prototype `relais` (7.1).

**Enseignements**

1. **WebView2 est disponible** sur le poste Windows de test, avec un
   compte standard : la console et le panneau élève peuvent être des
   applications Tauri.
2. **Signature indispensable** : après Avast (1.1, 2.1), Defender
   signale à son tour les exécutables non signés.
3. **Le relais dans la classe se heurte au pare-feu.** Une console qui
   embarque le relais reçoit des connexions : Windows demande alors une
   autorisation accompagnée d'un mot de passe, donc très probablement
   des droits d'administration. Sur un poste d'enseignant sans droits
   d'administration, les postes élèves ne pourraient pas joindre la
   console. À confirmer avec un compte standard dont le mot de passe
   diffère de celui de l'administrateur.
4. **Le panneau élève a aussi déclenché le pare-feu**, parce qu'il
   écoute les annonces du relais. Il vaut mieux inverser la découverte :
   le poste envoie une recherche sur le réseau local et le relais lui
   répond. Un poste élève n'aurait alors jamais rien à écouter.

**Limites**

- Poste Windows unique ; le cas d'un Windows sans WebView2 (Windows 10
  ancien) n'est pas testé. Le moteur peut s'installer par utilisateur,
  sans droits d'administration, à vérifier.
- Pas de paquet d'installation : les exécutables sont lancés tels quels.

**À reprendre dans l'implémentation**

- Découverte par recherche émise par le poste, réponse du relais.
- Autorisation du pare-feu accordée une fois à l'installation de la
  console (par un administrateur ou le service informatique), ou relais
  d'établissement ou hébergé quand elle est impossible.
- Signature du code dès les premières versions diffusées.

### 7.3 Prototype `antivirus`

Code (jetable) dans `prototypes/antivirus/`. Ce prototype ne produit pas
de binaire : il mesure la réaction des antivirus aux agents **déjà
construits** par les prototypes 2, 3 et 5, et établit la voie par
laquelle un logiciel légitime se fait reconnaître. Question et critère :
tableau de phase 2, ligne 6.

**Position de principe.** Surveo est libre et transparent (l'élève est
informé, le service informatique peut lire le code). On cherche donc à
**faire reconnaître** l'agent, pas à le dissimuler. L'empaquetage,
l'obscurcissement et l'anti-analyse sont écartés : contraires à la
transparence, mal notés par les éditeurs d'antivirus, et vains sur un
poste de salle géré. La voie retenue est la signature, la réputation, la
documentation et les exclusions décidées par l'établissement.

**Ce qui, dans l'agent, peut alerter un antivirus** — tout légitime,
chacun ressemblant pourtant à une famille d'indésirables : binaire
inconnu non signé (SmartScreen) ; capture d'écran répétée (`capture`) ;
lecture de texte pour le classifieur (`classifieur`, vu comme un
enregistreur de frappe — question ouverte SPEC.md 5.10) ; modification du
proxy de l'utilisateur et inscription au démarrage (`sans-admin`) ;
connexions sortantes vers le relais ; binaire MinGW (`capture.exe`),
réputation souvent mauvaise.

**Protocole** (détail dans `prototypes/antivirus/PROTOCOLE.md` et fiche
de test `fiche-test-windows.txt`). Les trois binaires non signés
(empreintes dans `empreintes.sha256`) sont soumis, sur Microsoft Defender
+ SmartScreen puis sur un antivirus d'établissement, dans trois états :
non signé, signé avec un certificat de test (effet d'une signature
valide, sans réputation — `signer-test.md`), signé avec le vrai
certificat du projet (effet signature + réputation). Résultats reportés
dans `resultats.md`, une ligne par poste × binaire × état de signature.
Les postes de salle sans droits d'administration (postes Région) se
testent sur place.

**Premiers constats déjà recueillis** par les autres prototypes, sur un
seul poste Windows, binaires non signés :

| Binaire | Antivirus | Réaction observée | Source |
|---|---|---|---|
| `capture.exe` | Avast | jugé suspect dès le lancement ; analyse non aboutie après 50 s | 1.1 |
| `sans-admin.exe` | Avast | exécutable non signé bloqué (désactivation nécessaire) | 2.1 |
| `sans-admin.exe` | Defender | aucune réaction | 2.1 |
| console et panneau élève (Tauri) | Defender / SmartScreen | jugés non sûrs (éditeur inconnu) | 7.2 |

Converge avec la question ouverte de VISION.md §7 et le critère de
crédibilité « exécutables signés » (§5) : **sur binaire non signé, le
blocage est déjà la règle**. La mesure qui reste à faire est celle du
binaire **signé**, condition du critère de réussite.

**Première passe systématique** (30 octobre 2026, poste Windows 11,
Defender 4.18.25080.5 + Avast 26.10.11201.1002, compte standard ;
relevé complet dans `prototypes/antivirus/resultats.md`) :

- SmartScreen signale les **trois** binaires non signés comme « éditeur
  non reconnu » — attendu ; mais la passe signé test (plus bas) montre
  qu'une signature quelconque ne suffit pas à le lever : il faut un
  certificat reconnu par SmartScreen et de la réputation.
- `capture.exe` : aucune réaction de l'antivirus (binaire déjà exécuté
  lors des tests précédents, donc en réputation locale ; à revérifier
  avec un hash neuf).
- `classifieur.exe` : envoyé à l'analyse cloud, sans blocage.
- `sans-admin.exe` : **mis en quarantaine par Avast sous le nom
  `IDP.Generic`, au moment où l'agent modifie le proxy de
  l'utilisateur** — pas au lancement. `IDP.Generic` est la détection
  comportementale générique d'Avast, connue pour ses faux positifs sur
  les binaires non signés qui touchent au réseau. C'est le blocage le
  plus dur observé jusqu'ici, et il confirme que **la modification du
  proxy par un binaire inconnu est le comportement le plus à risque**.

`IDP.Generic` étant une détection de réputation autant que de
comportement, une signature de test (auto-signée) ne la lèvera
probablement pas : seul le vrai certificat, plus la soumission du faux
positif à Avast, est attendu efficace. La signature de test reste utile
pour mesurer l'effet « signature valide » sur Defender et SmartScreen.

**Passe « signé test »** (même poste, 30 octobre 2026 ; certificat
auto-signé approuvé localement, signature `Valid` horodatée) : **aucun
effet favorable**. SmartScreen affiche toujours « Éditeur inconnu ».
Avast retient les trois binaires signés dans CyberCapture dès le
lancement puis refuse l'exécution, alors que le `capture.exe` non signé
déjà connu passe : la signature change le hash, et Avast juge sur la
réputation du hash, pas sur la signature. `sans-admin.exe` n'atteint
donc pas la pose du proxy : la persistance d'`IDP.Generic` reste à
mesurer. Defender, en mode passif derrière Avast sur ce poste, n'a pas
pu être mesuré.

**Passe « Defender actif »** (VM Windows 11 propre, sans antivirus tiers,
Defender actif, 30 octobre 2026 ; binaires non signés) : **aucun blocage
comportemental**. La capture d'écran, la pose du proxy (fixe **comme**
PAC) et l'inscription au démarrage ne déclenchent rien ;
`Get-MpThreatDetection` et `Get-MpThreat` sont vides. Le seul frein est
SmartScreen au premier lancement (« éditeur non reconnu »). Autrement
dit, le blocage dur sur la pose du proxy est **propre à Avast** ;
Defender, l'antivirus le plus répandu dans les établissements sans
produit tiers, laisse faire. Pour lui, la **signature et la réputation
suffisent**, il n'y a pas de comportement à corriger. (`classifieur.exe`
n'a pas pu s'exécuter, `onnxruntime.dll` absent du kit de transfert ; le
binaire a été chargé sans blocage mais sa lecture de texte n'a pas
tourné — à revérifier avec le bundle complet.)

**Voie de reconnaissance visée** (à confirmer par les mesures signées) :

1. **Signer en Authenticode**, avec horodatage, tous les exécutables
   distribués. Certificat **EV** = réputation SmartScreen immédiate ;
   **OV** = réputation qui monte avec les installations.
2. **Stabiliser la réputation** : éditeur constant, empreintes publiées,
   **soumission des binaires aux éditeurs** comme connus et sains
   (portail Microsoft ; formulaires de faux positif des autres).
3. **Documenter pour le service informatique** (comportement, chemins,
   empreintes, builds reproductibles — SPEC.md 7.10) pour qu'une
   exclusion soit posée en connaissance de cause.
4. **Mode déployé** : exclusion posée par l'établissement (stratégie de
   groupe, console de l'antivirus) ; voie la plus robuste.
5. **Module sensible** : rappeler que l'analyse est locale, que rien
   n'est exfiltré et que le modèle n'est pas livré par défaut (SPEC.md
   5.10) — argument d'acceptation.

**Ce qui reste à faire.** Defender actif et Avast ont été mesurés, en non
signé et en signé de test. La dernière inconnue est le **vrai
certificat** (autorité reconnue) : lève-t-il, avec le temps et le volume,
l'avertissement SmartScreen, et évite-t-il le blocage CyberCapture et
`IDP.Generic` d'Avast, après soumission du faux positif ? Restent aussi
non couverts un antivirus d'entreprise et un vrai poste de salle. Le
`classifieur.exe` est à retester avec son `onnxruntime.dll`. L'essentiel
est acquis — Defender ne bloque aucun comportement, Avast ne bloque que
sur réputation de hash — et la décision de spécification est portée par
SPEC.md (INST-7 et la décision datée du 7.9 : binaires signés,
reconnaissance par signature, réputation, documentation et exclusions de
l'établissement, jamais par contournement). La vérification au vrai
certificat reste à mener au jalon crédibilité (VISION.md §5).

### 7.4 Relais, découverte et installation de l'agent

Réalisés pour la séance minimale (1.2).

**Relais** (`crates/surveo-relay-core`), repris du prototype `relais`
avec ce que celui-ci laissait « à reprendre » :

- Il écoute sur le port TCP 47824, ou sur un port libre s'il est pris.
  Il accepte les consoles et les postes de son établissement, et route :
  une commande de console vers toute la salle visée ou les postes
  désignés, un message de poste vers toutes les consoles.
- Files bornées : 256 messages de contrôle par client ; pour les
  miniatures, seule la plus récente de chaque poste attend d'être remise
  à une console lente.
- Ping toutes les 10 s ; un client dont rien n'arrive pendant 30 s est
  tenu pour parti. Un poste qui se reconnecte remplace son ancienne
  connexion, sans annonce de départ.
- La console qui l'embarque s'y attache directement
  (`Relay::attach_console`) et reçoit les mêmes trames qu'une console
  distante, sans passer par le réseau.

**Découverte** (`crates/surveo-net/src/discovery.rs`, DECOUV-1) : l'agent
essaie le dernier relais joint (`relais.json`), puis les adresses que la
console a inscrites dans le paquet, puis une recherche sur le réseau
local : il diffuse un datagramme UDP (port 47823) portant l'identifiant de
son établissement, sur l'adresse de diffusion de chacune de ses
interfaces, la diffusion générale et l'interface locale. Le relais de cet
établissement répond directement, avec son port. Le poste n'écoute
jamais : la réponse à un envoi passe les pare-feux. Un relais trouvé n'est
suivi que s'il s'authentifie au nom de l'établissement (DECOUV-2). Les
tentatives suivent un délai croissant de 1 à 10 s. Les niveaux 2
(indication publiée sur le réseau de l'établissement) et 4 (annuaire de
l'offre hébergée) ne sont pas faits.

**Installation de l'agent** (`apps/agent/src/setup.rs`, sans droits
d'administration) : lancé depuis un dossier qui contient
`surveo-rattachement.json`, l'agent se copie, avec le paquet et
`WebView2Loader.dll` sous Windows, dans son dossier
(`%LOCALAPPDATA%\Surveo\Agent`, `~/.local/share/surveo/agent`),
s'inscrit au démarrage de session (clé `Run` de l'utilisateur, fichier
`~/.config/autostart/surveo-agent.desktop`) et se relance depuis là. Un
verrou (`agent.lock`, qui porte le numéro du processus) garantit un seul
agent par compte. `--desinstaller` arrête l'agent en cours, retire
l'inscription et efface le dossier, donc le rattachement (INST-5) ; sous
Windows, l'effacement est différé de quelques secondes quand l'exécutable
installé se désinstalle lui-même. Options d'essai : `--sur-place` (sans
installation), `--dossier` (plusieurs agents sur un même poste),
`--ecran-factice N` (miniatures de démonstration).


## 9. Interface et ergonomie

### 9.1 Charte visuelle

La charte est celle de la maquette validée (`maquette/console-surveo.html`),
reprise par les interfaces dans `ui/shared/base.css` sous forme de
variables CSS : la maquette et les interfaces partagent les mêmes noms et
les mêmes valeurs.

- **Couleurs** : palette de GitHub, en thème clair et sombre classique,
  suivant le réglage du système (ACCES-3). Sombre : fond `#0d1117`,
  surfaces `#161b22` et `#1c2128`, traits `#30363d`, texte `#e6edf3`,
  accent bleu `#58a6ff`. Clair : fond `#f6f8fa`, surfaces blanches,
  accent `#0969da`. Les états ont chacun leur couleur (vert « dans la
  séance », ambre « aide », rouge « retard ou perte », violet
  « restriction »), toujours doublée d'une icône et d'un texte
  (ACCES-4). La barre d'actions et les notifications restent sombres
  dans les deux thèmes.
- **Polices** : Atkinson Hyperlegible pour le texte, IBM Plex Mono pour
  les numéros et les heures, livrées avec l'application.
- **Icônes** : celles de la maquette, tracées au trait
  (`ui/shared/Icon.svelte`).

**Décision** (31 octobre 2026, demande de l'utilisateur après les premiers
essais) : la palette vert-gris d'origine est remplacée par celle de
GitHub, dans la maquette puis dans les interfaces.
