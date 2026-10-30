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
| 3. Architecture | Composants, protocole, organisation du code, à partir des résultats de la phase 2 | Proposée, à valider ([Architecture](#architecture)) |
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
  (`~/.config/surveo`, `%LOCALAPPDATA%\Surveo`).
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

Un espace de travail Cargo (workspace) ; les binaires réutilisent des
crates communes. Noms indicatifs :

| Crate | Rôle |
|---|---|
| `surveo-protocol` | Définition et sérialisation des messages, version du protocole. Partagée par tous. |
| `surveo-identity` | Clé d'établissement, certificats de membre, défi mutuel, liaison TLS (6.1). |
| `relay_core` | Moteur de routage du relais (existe, prototype `tauri`). |
| `surveo-platform` | Capacités par OS (capture, proxy, démarrage, pause, applications) derrière des traits. |
| `surveo-agent` | Agent du poste élève ; utilise `platform`, `protocol`, `identity`, les classifieurs. |
| `surveo-relay` | Binaire relais autonome (utilise `relay_core`). |
| `surveo-console` | Cœur Rust de la console Tauri ; embarque `relay_core` en option. |

- **Interfaces TypeScript** : console (plan de salle, modes, conduite de
  séance) et panneau élève. La maquette validée
  (`maquette/console-surveo.html`) sert de référence. Le cadre TS (Svelte,
  React…) reste à choisir (questions ouvertes).
- **Types partagés Rust ↔ TypeScript** : les messages du protocole et les
  objets d'IPC Tauri doivent exister des deux côtés ; on les **engendre**
  depuis Rust (par exemple `ts-rs` ou `typeshare`) pour qu'ils restent
  synchronisés (question ouverte : lequel).

### Ordre de réalisation (phase 4)

Développement par incréments **complets** (verticaux, utilisables),
chacun du poste à la console :

1. **Socle** : workspace, `surveo-protocol`, `surveo-identity` tirés des
   prototypes `relais`/`tauri` ; `relay_core` promu hors des prototypes.
2. **Séance minimale** : l'agent se connecte et se place ; la console
   montre le plan et les miniatures ; ouvrir, prolonger, clôturer ;
   retour à l'état d'origine (chap. 1). Première tranche de bout en bout.
3. **Maîtrise** : écran en pause, filtrage web, fermeture d'application,
   mode examen, via `surveo-platform` (chap. 2).
4. **Pédagogie** : modèles de TP, lancement et suivi, rendus,
   distribution, sondages (chap. 3).
5. **Communication** : messages, conversation, demande d'aide, diffusion
   d'écran (chap. 4).
6. **Rôles et confiance** : appairage, rôles, révocation, journal
   d'audit ; configuration poussée signée ; mises à jour signées
   (chap. 6, 7).
7. **IA** : classifieurs locaux, IA générative via le relais (chap. 5).
8. **Données et RGPD** : conservation, traçabilité en option, kit RGPD
   (chap. 8).
9. **Durcissement et diffusion** : signature du code et soumission aux
   éditeurs d'antivirus (INST-7, 7.3), paquets d'installation, deux modes
   de déploiement.

### Décisions

30 octobre 2026 : architecture proposée, à valider.

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

### Questions ouvertes

- Format des messages de contrôle : garder JSON (lisible, souple) ou
  passer à un format compact (CBOR/MessagePack via `serde`) ?
- Cadre TypeScript des interfaces (Svelte, React, autre) et outil de
  génération des types depuis Rust (`ts-rs` vs `typeshare`).
- Report pendant une déconnexion : la courte file de messages de séance
  est-elle tenue par le relais ou seulement par la console responsable ?
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
