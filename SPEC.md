# Surveo — spécification

Ce document dit ce que fait Surveo, vu de l'utilisateur : règles,
messages, critères d'acceptation, décisions. Il est rangé par thème.
La réalisation est décrite dans IMPL.md.

Chaque règle porte un identifiant (`SEANCE-3`, `RESTR-1`…) pour que le
code et les tests puissent y renvoyer.

## Chapitres

1. [Séance et salle](#1-séance-et-salle)
2. [Maîtrise de la classe](#2-maîtrise-de-la-classe) (écran, filtrage web, applications, mode examen)
3. [Pédagogie](#3-pédagogie) (TP, rendus, distribution de fichiers, sondages)
4. [Communication](#4-communication) (messages, chat, demande d'aide, diffusion d'écran)
5. [IA](#5-ia) (classifieurs locaux, détection de harcèlement et de détresse, IA générative)
6. [Rôles et confiance](#6-rôles-et-confiance) (rôles, confiance sans inscription, appairage, audit)
7. [Déploiement et relais](#7-déploiement-et-relais) (installation, relais, découverte, reconfiguration, mises à jour)
8. [Données et RGPD](#8-données-et-rgpd) (minimisation, conservation, traçabilité, hébergement, droits)
9. [Interface et ergonomie](#9-interface-et-ergonomie) (espaces, sélection et actions, retours, mode projection, côté élève, accessibilité)

---

## 1. Séance et salle

### 1.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Poste** | Ordinateur sur lequel l'agent Surveo est installé. Il garde la même identité même si son nom ou son adresse réseau changent ; son nom d'ordinateur sert seulement de libellé. |
| **Poste de salle** | Poste appartenant à l'établissement, rattaché à une salle. |
| **Poste personnel** | Poste de l'élève (à la maison, par exemple), utilisé pour l'enseignement à distance. |
| **Salle** | Lieu physique qui regroupe des postes de salle. |
| **Plan de salle** | Position de chaque poste de salle dans la salle. |
| **Séance** | Période pendant laquelle un enseignant conduit une classe, dans une salle ou à distance, avec une heure de fin prévue. |
| **Console** | Application de l'enseignant. |
| **Restriction** | Tout changement de comportement d'un poste demandé pendant une séance : écran mis en pause, filtrage web, application fermée, etc. |
| **État d'origine** | Les réglages qu'avait le poste avant que Surveo n'applique une restriction. |

Le type d'un poste (de salle ou personnel) est choisi à l'installation.

### 1.2 Avant la séance

- **SALLE-1** — Dès qu'un enseignant choisit une salle, la console montre
  ses postes de salle, avant même qu'une séance soit ouverte. Chaque
  poste apparaît dans l'un de ces états : *éteint* (connu mais absent),
  *allumé*, *dans la séance*.
- **SALLE-2** — Le plan de salle appartient à la salle, pas à
  l'enseignant. Un enseignant qui arrive dans une salle déjà configurée
  retrouve le plan tel quel, sans rien configurer.
- **SALLE-3** — Un poste qui n'est pas encore placé sur le plan apparaît
  dans une zone « postes à placer ». L'enseignant le place en le
  glissant sur le plan ; la position est enregistrée avec la salle.
- **SALLE-4** — L'action « identifier » affiche sur l'écran du poste,
  pendant quelques secondes, un grand numéro, le même que celui de la
  tuile dans la console. Elle sert à retrouver un poste dans la salle
  réelle pour le placer sur le plan.

### 1.3 Ce que la console affiche

- **AFF-1** — La vue par défaut est le **plan de salle** : une tuile par
  poste, placée comme dans la salle réelle.
- **AFF-2** — Une tuile montre l'état du poste (SALLE-1), le ou les noms
  associés pour la séance (ELEVE-1) et quelques indicateurs : avancement
  du travail, fichier rendu, demande d'aide, restriction en cours.
- **AFF-3** — Modes d'affichage disponibles :

  | Mode | Usage |
  |---|---|
  | Plan | Vue par défaut : présence, position, état |
  | Liste | Trier et filtrer (qui n'a pas rendu, qui est en retard…) |
  | Mosaïque | Voir les écrans de tous les postes |
  | Focus | Un poste en grand |

- **AFF-4** — L'image de l'écran est une couche d'information qu'on
  active ou non sur les vues plan et liste. Elle n'est pas affichée par
  défaut.

### 1.4 Ouvrir, prolonger, clôturer une séance

- **SEANCE-1** — Pour ouvrir une séance, l'enseignant choisit une salle,
  ou « à distance », et une heure de fin.
- **SEANCE-2** — Une salle a au plus une séance ouverte à la fois.
  D'autres consoles peuvent se joindre à une séance ouverte ; leurs
  droits sont définis au chapitre « Rôles et confiance ».
- **SEANCE-3** — L'enseignant peut prolonger une séance en cours.
- **SEANCE-4** — Une séance ne peut pas dépasser une durée maximale,
  prolongations comprises.
- **SEANCE-5** — L'action « clôturer la séance » met fin à la séance et
  remet tous les postes dans leur état d'origine (RESTR-5).
- **SEANCE-6** — Une séance non clôturée se termine d'elle-même à l'heure
  de fin prévue, plus une marge.

### 1.5 Rejoindre une séance

- **REJ-1** — Un poste de salle rejoint automatiquement la séance ouverte
  dans sa salle. S'il n'y en a pas, il attend et apparaît comme
  *allumé* dans la console.
- **REJ-2** — Un poste personnel ne rejoint une séance que par un geste
  de l'élève (code ou lien de séance), à chaque séance. Il ne rejoint
  jamais une séance automatiquement.
- **REJ-3** — En dehors d'une séance, l'agent d'un poste personnel
  n'observe rien et n'envoie rien.
- **REJ-4** — Tant qu'un poste est dans une séance, un indicateur reste
  visible à l'écran :

  > Ce poste est relié à la séance de {enseignant} jusqu'à {heure de fin}.

- **REJ-5** — Une séance à distance ne peut être ouverte que si le
  relais est joignable depuis Internet. Sinon, la console affiche :

  > Ce relais n'est pas joignable depuis l'extérieur : les postes
  > personnels ne pourront pas rejoindre la séance.

### 1.6 Élèves et binômes

- **ELEVE-1** — Pour une séance, un poste porte zéro, un ou plusieurs
  noms d'élèves. Un binôme partage toujours un seul poste.
- **ELEVE-2** — Les noms sont saisis par l'élève en rejoignant la
  séance, ou par l'enseignant, soit à la main, soit en les glissant
  depuis une liste de classe.
- **ELEVE-3** — Les noms sont facultatifs : tout fonctionne sans eux. Un
  poste sans nom est désigné par son libellé.
- **ELEVE-4** — Les rendus et les réponses aux sondages d'un poste sont
  attribués à tous les noms de ce poste.
- **ELEVE-5** — L'association entre un poste et des noms ne vaut que pour
  la séance.

### 1.7 Restrictions et retour à l'état d'origine

Règle générale :

> **Un poste ne porte jamais de restriction au-delà de la fin prévue de
> la séance à laquelle il est relié. Un poste relié à aucune séance
> active est dans son état d'origine.**

- **RESTR-1** — Toute restriction est liée à une séance. Elle cesse au
  plus tard à la fin prévue de la séance, marge comprise (SEANCE-6).
- **RESTR-2** — Une perte de connexion avec le relais ne lève pas les
  restrictions : le poste les garde jusqu'à la fin prévue de la séance
  et se resynchronise quand la connexion revient.
- **RESTR-3** — Au démarrage, un poste revient toujours à son état
  d'origine. S'il rejoint ensuite une séance en cours, il en réapplique
  les restrictions.
- **RESTR-4** — Un poste ne restaure que ce qu'il a lui-même modifié, en
  remettant la valeur qu'il avait relevée avant la modification. Il
  n'écrase jamais un réglage décidé par ailleurs (par le service
  informatique, par exemple).
- **RESTR-5** — À la clôture, la console affiche un récapitulatif des
  remises en état. Les postes injoignables y sont signalés :

  > {poste} : injoignable, remise en état à son prochain démarrage ou à
  > {heure de fin}.

- **RESTR-6** — Aucune restriction ne s'applique à un poste personnel
  (voir 1.8).

### 1.8 Poste personnel

- **PERSO-1** — Sur un poste personnel, Surveo se limite à **observer et
  signaler** : pédagogie (TP, sondages, distribution et rendu de
  fichiers, messages) et vue de l'écran, avec l'indicateur REJ-4
  toujours affiché.
- **PERSO-2** — Aucun outil technique (console de commandes, fichiers,
  processus, extinction) n'est disponible sur un poste personnel.

### 1.9 Critères d'acceptation

1. Un enseignant qui ouvre la console dans une salle déjà configurée
   voit le plan et les postes allumés sans rien configurer.
2. Un nouveau poste apparaît dans « postes à placer » ; « identifier »
   affiche son numéro sur son écran ; une fois placé, il garde sa place
   pour les séances suivantes, quel que soit l'enseignant.
3. Pendant une séance avec filtrage web, un élève éteint son poste sans
   rien fermer. Au démarrage suivant, sans séance ouverte, le poste n'a
   aucun filtrage.
4. Une coupure réseau de cinq minutes pendant une séance ne lève aucune
   restriction ; le poste réapparaît dans la séance sans action de
   l'enseignant.
5. Un enseignant oublie de clôturer : à l'heure de fin prévue plus la
   marge, plus aucun poste ne porte de restriction.
6. Un réglage de proxy défini par le service informatique est intact
   après une séance qui a appliqué puis levé un filtrage web.
7. Sur un poste personnel, rien n'est observé avant que l'élève ait
   rejoint la séance, l'indicateur est visible pendant toute la séance,
   et rien n'est observé après sa fin.
8. Toutes les fonctions de la séance marchent sans qu'aucun nom d'élève
   n'ait été saisi.

### 1.10 Décisions

25 octobre 2026 :

- L'identité de base est le **poste**, pas l'élève, car les salles sont
  souvent déjà configurées et les élèves partagent parfois un compte
  unique. Les noms d'élèves sont une couche facultative, par séance.
- La vue par défaut est le plan de salle ; la mosaïque d'écrans n'est
  qu'un mode parmi d'autres.
- Les restrictions sont limitées dans le temps par la séance. La perte
  de connexion ne les lève pas, car les connexions peuvent être
  intermittentes.
- Le type de poste (salle ou personnel) est fixé à l'installation. Un
  poste personnel ne rejoint une séance que sur un geste de l'élève.
- Sur un poste personnel, on se limite à observer et signaler, sans
  restriction.
- Un binôme partage toujours un seul poste.
- Un poste de salle allumé attend la séance et l'enseignant le voit
  avant d'ouvrir la séance.

### 1.11 Questions ouvertes

- Valeurs par défaut : durée d'une séance, durée maximale (SEANCE-4),
  marge (SEANCE-6). Proposition : 1 h, 4 h, 10 min, la durée maximale
  étant réglable par l'établissement.
- Qui peut modifier le plan de salle (chapitre « Rôles et confiance »).
- Un élève peut-il quitter de lui-même une séance sur un poste
  personnel ? Proposition : oui, et la console le signale.
- Séance hybride : une même séance peut-elle réunir des postes de salle
  et des postes personnels ?
- Liste de classe : où elle est conservée, et comment on l'importe
  (fichier, ENT).

---

## 2. Maîtrise de la classe

Ce chapitre regroupe les actions par lesquelles l'enseignant cadre
l'usage des postes pendant la séance : mettre les écrans en pause,
filtrer l'accès au web, fermer ou interdire des applications, passer la
séance en mode examen.

Toutes ces actions sont des **restrictions** au sens du chapitre 1 :
elles obéissent aux règles RESTR-1 à RESTR-6. En particulier, elles sont
liées à la séance, ne survivent jamais à sa fin prévue, et ne
s'appliquent pas à un poste personnel (RESTR-6, voir 2.7).

### 2.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Écran en pause** | Écran de l'élève recouvert par Surveo, affichant un message, le temps que l'enseignant le décide. |
| **Filtrage web** | Limitation des sites accessibles depuis le poste pendant la séance. |
| **Liste d'autorisation** | Ensemble des sites accessibles ; tout le reste est bloqué. |
| **Liste de blocage** | Ensemble des sites bloqués ; tout le reste est accessible. |
| **Application interdite** | Application que l'enseignant ne veut pas voir ouverte pendant la séance. |
| **Mode examen** | État de la séance où l'accès est restreint à l'essentiel, pour une évaluation. |
| **Mode de déploiement** | Manière dont l'agent est installé sur un poste : sans droits d'administration, ou déployé par l'établissement (voir 2.8). |

### 2.2 Principe

> **Une restriction est toujours visible par l'élève, bornée par la
> séance, et réversible. Surveo ne cache jamais à l'élève qu'une
> restriction s'applique.**

- **MAIT-1** — Quand une restriction s'applique à un poste, l'élève en
  est informé sur son poste (message, bandeau ou icône selon le cas).
  C'est le prolongement de l'indicateur de séance (REJ-4).
- **MAIT-2** — L'enseignant applique et lève chaque restriction à tout
  moment, sur tous les postes de la séance ou sur une sélection.
- **MAIT-3** — Ce qu'une restriction peut réellement garantir dépend du
  mode de déploiement du poste (2.8). La console indique ce mode pour
  chaque poste, afin que l'enseignant sache sur quoi il peut compter.
- **MAIT-4** — Toute restriction appliquée pendant une séance est
  consignée (qui, quoi, quand), pour le journal d'audit du chapitre
  « Données et RGPD ». Cela protège aussi l'enseignant en cas de
  contestation.

### 2.3 Écran en pause

- **ECRAN-1** — L'enseignant met en pause l'écran des postes de la
  séance ou d'une sélection. Pendant la pause, l'écran est recouvert et
  l'élève ne peut plus agir sur son poste.
- **ECRAN-2** — L'écran en pause affiche un message. Par défaut :

  > Écran mis en pause par {enseignant}.

  L'enseignant peut y mettre son propre texte (« Regardez le tableau »).
- **ECRAN-3** — L'enseignant lève la pause. Elle est aussi levée à la
  fin de la séance (RESTR-1) et, en cas de redémarrage du poste, ne
  revient pas de force si l'enseignant l'a entre-temps levée.
- **ECRAN-4** — La mise en pause n'interrompt pas le travail en cours
  sur le poste : rien n'est fermé, le travail de l'élève reste tel quel
  en attendant la reprise.

### 2.4 Filtrage web

- **WEB-1** — L'enseignant limite l'accès au web pendant la séance, selon
  l'un de ces réglages :
  - **liste d'autorisation** : seuls les sites indiqués sont accessibles ;
  - **liste de blocage** : les sites indiqués sont bloqués, le reste
    reste accessible ;
  - **tout bloquer** : aucun site accessible (« on ferme les écrans »).
- **WEB-2** — Quand un site est bloqué, l'élève en est informé par une
  notification de Surveo sur son poste :

  > {site} n'est pas accessible pendant la séance de {enseignant}.

  Pour un site en HTTP, la page elle-même affiche ce message. Pour un
  site en HTTPS, le navigateur affiche sa propre erreur, que la
  notification explique.

- **WEB-3** — Le filtrage ne vaut que pour la séance : à sa fin, l'accès
  d'origine du poste est rétabli (RESTR-1, RESTR-3). Surveo ne touche
  pas à un filtrage déjà en place sur le poste (RESTR-4).
- **WEB-4** — L'enseignant enregistre des listes réutilisables (par
  exemple « sites du TP réseau »), qui le suivent d'une salle à l'autre.

### 2.5 Applications

- **APP-1** — L'enseignant voit les applications ouvertes sur un poste
  (AFF-2) et peut en fermer une. L'élève est prévenu de la fermeture :

  > {application} a été fermée par {enseignant}.

- **APP-2** — L'enseignant définit une liste d'applications interdites
  pendant la séance (jeux, par exemple). Une application interdite qui
  s'ouvre est fermée, et l'élève en est informé comme en APP-1.
- **APP-3** — Surveo ne ferme jamais une application du système ni une
  application nécessaire au travail demandé : la liste d'interdiction ne
  porte que sur des applications nommées par l'enseignant.
- **APP-4** — Les listes d'applications interdites sont réutilisables et
  suivent l'enseignant, comme les listes de sites (WEB-4).
- **APP-5** — La fermeture d'applications ne vaut que pour la séance :
  après la séance, plus aucune application n'est fermée par Surveo.

### 2.6 Mode examen

- **EXAM-1** — Le mode examen est un état de la séance, activé et
  désactivé par l'enseignant, affiché en permanence à l'élève :

  > Séance d'examen en cours jusqu'à {heure de fin}.

- **EXAM-2** — En mode examen, par défaut :
  - l'accès web est en liste d'autorisation (WEB-1), vide par défaut
    (aucun site), l'enseignant ajoutant les sites permis ;
  - les applications interdites de la séance sont fermées (2.5) ;
  - aucune analyse par l'IA n'a lieu (chapitre 5) ;
  - la conversation élève↔enseignant reste possible (chapitre 4), pour
    signaler un problème ; les autres communications entre élèves
    restent, comme toujours, absentes.
- **EXAM-3** — Le mode examen a une heure de fin, comme la séance. Il
  s'arrête au plus tard à la fin de la séance et rétablit l'état
  d'origine du poste (RESTR-1).
- **EXAM-4** — Les rendus en mode examen suivent le chapitre 3, avec
  l'heure de dépôt qui fait foi (RENDU-8).
- **EXAM-5** — Sur un poste personnel, le mode examen ne restreint rien
  (2.7) : Surveo se limite à observer et à signaler à l'enseignant si le
  poste quitte le contexte de l'examen. L'enseignant est prévenu de
  cette limite au lancement.

### 2.7 Poste personnel

- **PERSO-3** — Aucune restriction de ce chapitre ne s'applique à un
  poste personnel (RESTR-6) : ni écran en pause, ni filtrage web, ni
  fermeture d'application. Sur ces postes, Surveo observe et signale
  seulement (PERSO-1).

### 2.8 Modes de déploiement

- **MODE-1** — Un poste fonctionne selon l'un de deux modes de
  déploiement, fixé à l'installation :
  - **sans droits d'administration** : l'agent est installé dans le
    compte de l'utilisateur. Les restrictions s'appliquent dans la
    session de l'élève (filtrage par le réglage de l'utilisateur,
    fermeture d'applications de sa session). C'est une maîtrise
    **coopérative** : un élève averti peut tenter de la contourner.
  - **déployé par l'établissement** : l'agent est installé par le
    service informatique. Les restrictions sont plus difficiles à
    contourner.
- **MODE-2** — La console indique, pour chaque poste, son mode de
  déploiement, afin que l'enseignant sache ce que la restriction
  garantit vraiment (MAIT-3).
- **MODE-3** — Si un poste sort d'une restriction (par exemple l'élève
  coupe le filtrage de sa session), la console le signale à
  l'enseignant plutôt que de le masquer.
- **MODE-4** — Le mode de déploiement ne change rien à la transparence
  (MAIT-1) ni au caractère temporaire des restrictions (RESTR-1) : il ne
  joue que sur leur robustesse.
- **MODE-5** — Sans droits d'administration, le filtrage web est posé de
  façon à cesser avec l'agent : si l'agent s'arrête, le poste retrouve
  aussitôt l'accès au web, et la console signale l'arrêt de l'agent
  comme une sortie de restriction (MODE-3).
- **MODE-6** — Sans droits d'administration, le filtrage web ne porte que
  sur les navigateurs. Les autres programmes (client de jeu, par
  exemple) n'y sont pas soumis, et la console le dit.

### 2.9 Critères d'acceptation

1. L'enseignant met en pause l'écran d'une sélection de postes avec un
   message personnalisé : les postes concernés affichent ce message et
   ne répondent plus aux actions de l'élève ; les autres postes ne sont
   pas touchés.
2. L'enseignant active un filtrage en liste d'autorisation : un site
   permis s'ouvre ; un autre site est refusé et l'élève reçoit la
   notification d'accès suspendu.
3. À la fin de la séance, le filtrage disparaît et un réglage de proxy
   qui préexistait sur le poste est intact (RESTR-4).
4. Une application interdite ouverte par un élève est fermée et l'élève
   en est informé ; après la séance, cette même application peut être
   ouverte sans que Surveo n'intervienne.
5. En mode examen, seuls les sites autorisés sont accessibles, aucune
   analyse par l'IA n'a lieu, et l'élève voit en permanence l'indication
   d'examen avec l'heure de fin.
6. Sur un poste personnel, activer le mode examen ou un filtrage ne
   restreint rien : la console le signale à l'enseignant.
7. La console affiche le mode de déploiement de chaque poste ; un poste
   sans droits d'administration dont l'élève a coupé le filtrage apparaît
   comme étant sorti de la restriction.

### 2.10 Décisions

29 octobre 2026 : chapitre proposé à partir de la version précédente,
à valider.

- Repris de la version précédente : écran recouvert avec message,
  filtrage web par proxy sans droits d'administration, « tout bloquer »
  (isolement réseau), listes d'applications interdites avec fermeture.
- Décisions :
  - Toute restriction est visible par l'élève (MAIT-1) et consignée
    (MAIT-4).
  - Le filtrage web propose liste d'autorisation, liste de blocage ou
    tout bloquer.
  - La fermeture d'applications remplace l'accès brut à la liste des
    processus pour l'enseignant ; la liste des processus relèvera du
    rôle technicien (chapitre 6).
  - Le mode examen est un état de la séance : liste d'autorisation vide
    par défaut, applications interdites fermées, IA coupée, heure de fin.
  - Deux modes de déploiement assumés (sans admin / établissement), avec
    affichage du mode par poste et signalement des sorties de
    restriction.
- 29 octobre 2026, après le prototype `sans-admin` (IMPL.md, 2.1) :
  - WEB-2 : l'élève est prévenu d'un blocage par une notification de
    Surveo, car une page explicative n'est possible qu'en HTTP.
  - MODE-5 et MODE-6 : sans droits d'administration, le filtrage cesse
    avec l'agent et ne porte que sur les navigateurs.

### 2.11 Questions ouvertes

- Qui définit les listes d'applications interdites et de sites : chaque
  enseignant, ou aussi l'établissement pour toute une salle (chapitre
  6) ?
- En mode examen, faut-il figer aussi la fermeture de la fenêtre du TP
  ou interdire le changement de compte de session ?
- Quand un élève sort d'une restriction sur un poste sans droits
  d'administration (MODE-3), faut-il une simple alerte, ou aussi une
  trace dans le journal d'audit ?
- Faut-il un réglage « liste d'autorisation » commun à tout
  l'établissement, inspiré des listes de référence des ENT ?

---

## 3. Pédagogie

### 3.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Modèle de TP** | Description réutilisable d'un TP : titre, étapes, fichiers joints, nombre de rendus attendus. Il appartient à l'enseignant. |
| **TP** | Un modèle lancé dans une séance, sur un ensemble de postes. |
| **Étape** | Partie d'un TP : un objectif, des consignes, une durée indicative et une action en cas de dépassement. |
| **Rendu** | Fichier déposé par un élève pour un TP. |
| **Distribution** | Envoi de fichiers par l'enseignant vers des postes. |
| **Sondage** | Question posée pendant la séance, dont l'enseignant recueille les réponses. |
| **Bibliothèque** | Ensemble des modèles de TP et des sondages d'un enseignant. |

### 3.2 Modèles de TP

- **MODELE-1** — L'enseignant crée, modifie, duplique et supprime ses
  modèles. Un modèle contient un titre, une liste ordonnée d'étapes, des
  fichiers joints (facultatifs) et un nombre de rendus attendus (zéro si
  le TP n'attend aucun rendu).
- **MODELE-2** — Une étape comprend :
  - un objectif, court ;
  - des consignes en texte mis en forme (titres, gras, listes, code) ;
  - une durée indicative ;
  - une action en cas de dépassement : *rien*, *afficher les consignes*
    ou *aide de l'IA* (chapitre 5).
- **MODELE-3** — La bibliothèque suit l'enseignant : ses modèles sont
  disponibles quelle que soit la salle.
- **MODELE-4** — Un modèle s'exporte et s'importe sous forme d'un
  fichier unique, fichiers joints compris, pour le sauvegarder ou le
  partager avec un collègue.
- **MODELE-5** — L'IA peut proposer un modèle à partir d'un cours ou
  d'une description (chapitre 5). La proposition s'ouvre en
  modification : rien n'est lancé sans relecture de l'enseignant.
- **MODELE-6** — Toutes les fonctions de ce chapitre marchent sans IA.

### 3.3 Lancer et suivre un TP

- **TP-1** — L'enseignant lance un modèle dans la séance ouverte, sur
  tous les postes de la séance ou sur une sélection. Le TP est une copie
  du modèle : modifier le modèle ensuite ne change pas le TP en cours.
- **TP-2** — Un TP lancé sur toute la séance est aussi envoyé aux postes
  qui rejoignent la séance plus tard.
- **TP-3** — Un poste qui redémarre ou perd la connexion retrouve, en
  rejoignant la séance, son TP dans l'état où il l'avait laissé : étapes
  terminées et rendus déjà envoyés.
- **TP-4** — Un poste a au plus un TP en cours. Lancer un autre TP sur
  ce poste remplace le premier, après confirmation de l'enseignant ; les
  rendus déjà reçus sont conservés.
- **TP-5** — Chez l'élève, la fenêtre du TP montre :
  - le titre et la liste des étapes ;
  - les consignes de l'étape en cours ;
  - le temps indicatif restant pour l'étape en cours ;
  - les fichiers joints, qu'il peut ouvrir ;
  - une zone de dépôt des rendus et le compte des rendus envoyés.
- **TP-6** — Seul l'élève marque une étape comme terminée, et il peut
  revenir sur ce choix ; l'enseignant ne valide pas les étapes. L'étape
  en cours est la première étape non terminée.
- **TP-7** — La durée d'une étape est indicative : son dépassement ne
  bloque jamais l'élève. Il déclenche l'action prévue (MODELE-2) et le
  poste apparaît *en retard* dans la console.
- **TP-8** — Dans la console, chaque poste indique son étape en cours,
  un éventuel retard et le nombre de rendus reçus sur le nombre attendu
  (AFF-2). Une synthèse donne le nombre de postes à chaque étape.
- **TP-9** — L'enseignant peut arrêter le TP sur tous les postes ou sur
  une sélection. La fin de la séance arrête tous les TP ; la fenêtre de
  l'élève affiche alors :

  > Séance terminée. Vos rendus envoyés ont bien été reçus.

  ou, s'il reste des rendus en attente (RENDU-3) :

  > Séance terminée. {n} rendu(s) seront envoyés dès que la connexion
  > avec l'enseignant sera rétablie.

### 3.4 Rendus

- **RENDU-1** — L'élève rend un fichier en le glissant dans la fenêtre du
  TP ou en le choisissant. Son fichier reste en place : Surveo en envoie
  une copie.
- **RENDU-2** — L'élève ne voit l'accusé de réception qu'une fois le
  rendu effectivement reçu côté enseignant :

  > « {fichier} » reçu par l'enseignant à {heure}.

- **RENDU-3** — Si la connexion est interrompue, le rendu est mis en
  attente et envoyé automatiquement au retour de la connexion :

  > « {fichier} » en attente d'envoi : la connexion avec l'enseignant
  > est interrompue.

  Un rendu déposé avant la fin de la séance est accepté même s'il
  arrive après (RENDU-8).
- **RENDU-4** — Le nombre de rendus attendus est un repère (« 2 / 3 ») :
  l'élève peut en déposer davantage.
- **RENDU-5** — Si l'élève rend de nouveau un fichier du même nom, toutes
  les versions sont conservées et horodatées ; la plus récente sert de
  référence.
- **RENDU-6** — Côté enseignant, les rendus sont rangés par séance, puis
  par TP, puis par poste. Le dossier d'un poste porte les noms associés
  au poste (ELEVE-4), ou à défaut son libellé. La console ouvre ce
  dossier en un clic.
- **RENDU-7** — Un fichier qui dépasse la taille maximale est refusé :

  > « {fichier} » est trop volumineux ({taille}) : la limite est de
  > {limite}.

- **RENDU-8** — Un travail se rend dans le délai de la séance : c'est
  l'heure du dépôt qui compte. Après la fin de la séance, la zone de
  dépôt est fermée.

### 3.5 Distribution de fichiers

- **DISTRIB-1** — L'enseignant envoie un ou plusieurs fichiers à tous les
  postes de la séance ou à une sélection, en les glissant sur le plan,
  sur une tuile, ou par un bouton.
- **DISTRIB-2** — Les fichiers arrivent dans un dossier Surveo de
  l'élève, avec un sous-dossier par séance. Un fichier existant n'est
  jamais écrasé : le fichier reçu est renommé.
- **DISTRIB-3** — L'élève est prévenu et peut ouvrir le fichier ou le
  dossier :

  > {enseignant} vous a envoyé « {fichier} ».

- **DISTRIB-4** — La console indique, pour chaque poste, si les fichiers
  sont reçus ou en attente. Un poste déconnecté les reçoit en rejoignant
  la séance, tant qu'elle est ouverte.
- **DISTRIB-5** — Les fichiers joints à un TP sont distribués au lancement
  du TP, selon les mêmes règles.
- **DISTRIB-6** — Sur un poste de salle, les fichiers distribués sont
  retirés à la fin de la séance, comme les restrictions sont levées
  (RESTR-1). Un poste éteint à ce moment-là les retire à son démarrage
  suivant (RESTR-3).
- **DISTRIB-7** — Surveo ne retire que les fichiers qu'il a lui-même
  distribués. Les fichiers créés par l'élève ne sont jamais touchés.
- **DISTRIB-8** — Quelques minutes avant la fin de la séance, l'élève
  d'un poste de salle est prévenu :

  > La séance se termine à {heure de fin}. Les fichiers reçus pendant la
  > séance seront alors retirés de ce poste : pensez à rendre votre
  > travail.

- **DISTRIB-9** — Sur un poste personnel, les fichiers distribués restent
  à l'élève.

### 3.6 Sondages

- **SONDAGE-1** — Un sondage comporte une question et des réponses
  proposées, à choix unique ou à choix multiple.
- **SONDAGE-2** — L'enseignant lance un sondage sur tous les postes de la
  séance ou sur une sélection. La question s'affiche au premier plan chez
  l'élève. Tant que le sondage est ouvert, l'élève peut modifier sa
  réponse.
- **SONDAGE-3** — La console affiche les résultats en direct : graphique,
  taux de réponse, et détail par poste avec les noms associés.
- **SONDAGE-4** — L'enseignant clôt le sondage ; la question disparaît
  alors chez l'élève. Une durée limite peut être fixée au lancement.
- **SONDAGE-5** — L'enseignant peut montrer les résultats aux élèves.
  Les résultats montrés sont toujours agrégés et ne comportent aucun nom.
- **SONDAGE-6** — Les sondages sont enregistrés dans la bibliothèque de
  l'enseignant : il peut les relancer, les modifier, les exporter et les
  importer comme les modèles de TP (MODELE-4).
- **SONDAGE-7** — L'IA peut proposer des sondages à partir d'un cours
  (chapitre 5), avec relecture obligatoire comme pour MODELE-5.

### 3.7 Poste personnel

- **PEDA-1** — Toutes les fonctions de ce chapitre sont disponibles sur
  un poste personnel (PERSO-1).

### 3.8 Critères d'acceptation

1. Sans IA, un enseignant crée un modèle de trois étapes et le lance sur
   toute la séance : chaque poste affiche le TP et la console montre
   l'étape en cours de chaque poste.
2. Un poste redémarre en plein TP : après reconnexion, il retrouve ses
   étapes terminées et le compte de ses rendus.
3. Un poste qui rejoint la séance après le lancement reçoit le TP et ses
   fichiers joints.
4. Un élève rend un fichier pendant une coupure réseau : il voit « en
   attente d'envoi », le fichier arrive au retour de la connexion, et
   l'accusé de réception n'apparaît qu'à ce moment-là.
5. Un élève rend deux fois un fichier du même nom : les deux versions
   sont conservées et horodatées.
6. Le rendu d'un binôme dont les noms sont A et B se trouve dans le
   dossier « A, B » de la séance.
7. Une distribution vers un poste qui possède déjà un fichier du même
   nom laisse ce fichier intact.
8. Le dépassement de la durée d'une étape ne bloque pas l'élève et le
   poste apparaît *en retard*.
9. Les résultats d'un sondage montrés aux élèves ne contiennent aucun
   nom.
10. Un modèle exporté puis importé par un collègue sur une autre console
    est identique, fichiers joints compris.
11. Après la fin de la séance, l'élève ne peut plus déposer de rendu ; un
    rendu déposé juste avant la fin, pendant une coupure, arrive quand
    même.
12. À la fin de la séance, les fichiers distribués ont disparu d'un poste
    de salle, y compris d'un poste qui était éteint (au démarrage
    suivant) ; un fichier créé par l'élève dans le même dossier est
    intact.

### 3.9 Décisions

25 octobre 2026 : chapitre proposé à partir de la version précédente,
à valider.

- Repris de la version précédente : modèles à étapes (objectif,
  consignes en Markdown, durée, action en cas de dépassement), fichier
  joint, nombre de rendus attendus, fenêtre élève avec étapes à cocher
  et minuteur, rendus par glisser-déposer, sondages à options,
  génération de TP et de sondages par l'IA.
- L'élève coche seul ses étapes ; l'enseignant ne les valide pas.
- Un document se rend dans le délai de la séance.
- Sur les postes de salle, les fichiers distribués sont retirés à la
  fin de la séance.
- Nouveautés proposées : export et import des modèles, reprise du TP
  après redémarrage, rendus mis en attente pendant une coupure,
  versions des rendus, accusé de réception réel, rangement des rendus
  par séance et par noms d'élèves, résultats de sondage montrés aux
  élèves sans noms.

### 3.10 Questions ouvertes

- L'enseignant peut-il modifier un TP en cours (ajouter du temps, une
  étape) ?
- Faut-il des réponses libres (texte court) dans les sondages ?
- Un fichier distribué que l'élève a modifié est-il retiré lui aussi ?
  Proposition : oui, puisque le travail doit avoir été rendu pendant la
  séance (RENDU-8) et que le prévenir (DISTRIB-8) laisse le temps de le
  faire.
- Partage des modèles entre collègues : un simple fichier suffit-il, ou
  faut-il une bibliothèque commune à l'établissement (voire publique,
  dans l'esprit des communs numériques éducatifs) ?

---

## 4. Communication

Toute la communication de ce chapitre se fait entre l'enseignant et les
élèves, pendant une séance. Elle passe par le relais de la séance : rien
ne transite directement d'un poste élève à un autre.

### 4.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Message** | Texte court envoyé par l'enseignant vers un ou plusieurs postes, affiché aussitôt chez l'élève. |
| **Message prédéfini** | Message réutilisable enregistré par l'enseignant. |
| **Conversation** | Échange suivi entre l'enseignant et un poste (ou l'ensemble des postes de la séance). |
| **Demande d'aide** | Signal qu'un élève envoie à l'enseignant depuis son poste, équivalent d'une main levée. |
| **Diffusion d'écran** | Partage en direct d'un écran (celui de l'enseignant, ou celui d'un élève) vers d'autres postes de la séance. |

### 4.2 Messages

- **MSG-1** — L'enseignant envoie un message à tous les postes de la
  séance, à une sélection, ou à un seul poste.
- **MSG-2** — Le message s'affiche aussitôt au premier plan sur le poste
  de l'élève, avec le nom de l'enseignant. L'élève le referme d'un geste.
- **MSG-3** — Un message peut renvoyer à un fichier distribué
  (DISTRIB-1) : l'élève ouvre le fichier ou son dossier depuis le
  message.
- **MSG-4** — L'enseignant enregistre, modifie et supprime des messages
  prédéfinis. La bibliothèque de messages le suit d'une salle à l'autre,
  comme celle des modèles de TP (MODELE-3).
- **MSG-5** — Un message envoyé à un poste momentanément déconnecté lui
  parvient lorsqu'il rejoint la séance, tant qu'elle est ouverte. Un
  message devenu sans objet à la fin de la séance n'est pas affiché.

### 4.3 Conversation

- **CHAT-1** — Chaque poste a une conversation privée avec l'enseignant.
  L'enseignant voit une conversation par poste ; l'élève ne voit que la
  sienne.
- **CHAT-2** — Les élèves ne communiquent pas entre eux par Surveo :
  il n'existe aucun canal de poste élève à poste élève.
- **CHAT-3** — L'enseignant peut écrire à toute la séance d'un coup
  (annonce). L'annonce apparaît dans la conversation de chaque poste,
  attribuée à l'enseignant.
- **CHAT-4** — L'élève est averti discrètement d'un nouveau message
  (par exemple une pastille), sans fenêtre imposée, pour ne pas couper
  son travail.
- **CHAT-5** — Les messages sont limités en longueur et en débit, pour
  éviter les abus. Au-delà, l'élève est prévenu :

  > Vous envoyez des messages trop vite : patientez un instant.

- **CHAT-6** — Une conversation ne vit que le temps de la séance. Son
  éventuelle conservation relève du chapitre « Données et RGPD ».

### 4.4 Demande d'aide

- **AIDE-1** — L'élève demande de l'aide depuis son poste d'un seul
  geste. Son poste fait alors apparaître l'indicateur « demande d'aide »
  dans la console (AFF-2).
- **AIDE-2** — L'ordre des demandes est conservé : l'enseignant voit qui
  a demandé en premier.
- **AIDE-3** — La demande se lève quand l'enseignant la marque comme
  traitée, ou quand l'élève l'annule lui-même.
- **AIDE-4** — L'élève voit l'état de sa demande : en attente, ou prise
  en compte par l'enseignant.

### 4.5 Diffusion d'écran

- **DIFF-1** — L'enseignant diffuse son écran, ou une partie de son
  écran, vers tous les postes de la séance ou vers une sélection.
- **DIFF-2** — Chez l'élève, la diffusion s'affiche dans une fenêtre
  qu'il peut agrandir. Il sait d'où vient la diffusion :

  > {source} partage son écran.

- **DIFF-3** — L'enseignant peut diffuser l'écran d'un élève vers les
  autres postes (pour montrer un travail à la classe). L'élève dont
  l'écran est diffusé en est toujours informé sur son poste pendant
  toute la durée de la diffusion.
- **DIFF-4** — La diffusion de l'écran d'un **poste personnel** vers
  d'autres postes exige l'accord explicite de l'élève, demandé à chaque
  diffusion. Sans accord, rien n'est diffusé.
- **DIFF-5** — L'enseignant arrête la diffusion. Elle s'arrête aussi à
  la fin de la séance. À l'arrêt, la fenêtre disparaît chez les élèves.
- **DIFF-6** — Pendant une diffusion, l'élève garde la main sur son
  poste : la diffusion n'impose rien d'autre qu'une fenêtre.

### 4.6 Poste personnel

- **COMM-1** — Messages (4.2), conversation (4.3), demande d'aide (4.4)
  et réception d'une diffusion (4.5) fonctionnent sur un poste personnel.
- **COMM-2** — La diffusion de l'écran d'un poste personnel est soumise
  à DIFF-4.

### 4.7 Critères d'acceptation

1. Un message envoyé à toute la séance apparaît sur chaque poste présent
   avec le nom de l'enseignant.
2. Un message envoyé à un poste déconnecté lui parvient dès qu'il
   rejoint la séance, si elle est encore ouverte.
3. Un élève ne dispose d'aucun moyen, dans Surveo, d'écrire à un autre
   élève.
4. Deux élèves demandent de l'aide : la console les présente dans
   l'ordre des demandes ; marquer la première comme traitée la retire
   sans toucher la seconde.
5. L'enseignant diffuse son écran à la séance : chaque poste l'affiche
   et l'élève peut continuer à se servir de son poste.
6. L'écran d'un élève diffusé à la classe fait apparaître, sur le poste
   de cet élève, l'indication que son écran est partagé, pendant toute
   la diffusion.
7. Sur un poste personnel, aucune diffusion de l'écran de l'élève ne
   part sans son accord explicite.
8. À la fin de la séance, toute diffusion en cours s'arrête et les
   conversations ne sont plus accessibles.

### 4.8 Décisions

25 octobre 2026 : chapitre proposé à partir de la version précédente,
à valider.

- Repris de la version précédente : messages affichés chez l'élève,
  messages prédéfinis, chat enseignant↔élève, diffusion de l'écran de
  l'enseignant, limites de longueur et de débit.
- Décision : **pas de canal élève à élève** dans Surveo. La
  communication est centrée sur l'enseignant, pour éviter les usages
  malveillants entre élèves.
- Décision : diffuser l'écran d'un élève est toujours visible par cet
  élève ; sur un poste personnel, cela exige son accord explicite.
- Nouveautés proposées : demande d'aide (main levée) avec file d'attente
  ordonnée, notification discrète d'un nouveau message plutôt qu'une
  fenêtre imposée.

### 4.9 Questions ouvertes

- Pièces jointes dans la conversation (image, fichier), ou texte seul ?
- L'enseignant peut-il figer l'écran des élèves pendant qu'il diffuse le
  sien, pour capter l'attention ? Cela relèverait du chapitre 2
  (maîtrise de la classe), pas de la communication.
- Un élève peut-il demander de l'aide de façon privée (visible du seul
  enseignant) plutôt que par un indicateur sur la tuile ? L'indicateur
  est déjà discret, mais la question se pose pour les salles où les
  élèves voient l'écran de l'enseignant.
- Réactions rapides de l'élève sans ouvrir la conversation (« compris »,
  « bloqué ») — utile, ou superflu ?

---

## 5. IA

Surveo utilise l'IA de deux façons distinctes : des **classifieurs qui
tournent sur le poste élève**, et une **IA générative atteinte par le
relais**. Les deux sont facultatives : tout le reste de Surveo
fonctionne sans IA.

### 5.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Classifieur local** | Modèle exécuté sur le poste élève, qui produit une étiquette (par exemple « activité de jeu ») sans que les données brutes ne quittent le poste. |
| **IA générative** | Service d'IA qui produit du texte (générer un TP, analyser un travail), atteint par le relais. |
| **Fournisseur d'IA** | Destination de l'IA générative : un modèle local, ou un service distant choisi par l'enseignant. |
| **Module sensible** | Traitement d'IA touchant à des données sensibles, soumis à un régime particulier (5.4). |
| **Alerte** | Signal remonté à l'enseignant par un classifieur, sans donnée brute. |

### 5.2 Principes

> **Ce qu'un classifieur analyse ne quitte pas le poste. Seule une
> étiquette ou une alerte remonte. Aucun contenu frappé au clavier n'est
> transmis ni conservé.**

- **IA-1** — Toutes les fonctions d'IA sont facultatives et peuvent être
  désactivées. Surveo reste pleinement utilisable sans elles.
- **IA-2** — Les classifieurs s'exécutent sur le poste élève. Les textes,
  frappes et captures analysés restent sur le poste ; seule une étiquette
  ou une alerte en sort (5.3, 5.4).
- **IA-3** — L'IA générative passe par le relais de l'établissement ;
  l'enseignant choisit le fournisseur (5.6). Un poste élève ne joint
  jamais directement un service d'IA.
- **IA-4** — Aucune analyse par l'IA n'a lieu pendant un examen (EXAM-2).
- **IA-5** — Ce que fait l'IA est visible : une alerte dit sur quoi elle
  porte, une suggestion est présentée comme telle, un contenu généré est
  relu avant usage (MODELE-5, SONDAGE-7).

### 5.3 Classifieurs locaux

- **CLASS-1** — Surveo peut exécuter sur le poste des classifieurs
  d'activité, qui alimentent les indicateurs de la console (AFF-2) :
  par exemple activité de jeu, ou nature de l'application au premier
  plan.
- **CLASS-2** — Un classifieur ne fait remonter qu'une étiquette et, le
  cas échéant, un degré de confiance. Les captures, titres de fenêtres,
  frappes et statistiques qui ont servi à l'analyse ne sortent pas du
  poste.
- **CLASS-3** — Une étiquette est une aide à la décision, non une preuve.
  La console la présente comme une indication, à charge pour
  l'enseignant de regarder (par exemple en affichant l'écran, chapitre
  1).
- **CLASS-4** — Les classifieurs d'activité sont actifs par défaut
  seulement s'ils ne touchent pas à des données sensibles ;
  l'établissement peut les désactiver.

### 5.4 Détection de harcèlement et de détresse

- **SENS-1** — La détection de harcèlement et de détresse est un
  **module sensible** : elle analyse le texte saisi par l'élève pour
  repérer des situations préoccupantes.
- **SENS-2** — Ce module est **désactivé par défaut**. Seul le rôle
  établissement peut l'activer (chapitre 6), car l'établissement est
  responsable de ce traitement. Désactivé, il n'occupe aucune ressource
  sur le poste : son modèle n'est pas livré avec l'agent, mais téléchargé
  depuis le relais quand l'établissement active le module.
- **SENS-3** — L'analyse se fait **sur le poste** (IA-2). Le texte frappé
  n'est ni transmis ni conservé. Seule une **alerte**, portant un niveau
  et un motif général (« propos potentiellement violents »), remonte.
- **SENS-4** — Quand il est activé, l'élève en est informé, au-delà de
  l'indicateur de séance (REJ-4), dans les conditions fixées par
  l'établissement (charte informatique).
- **SENS-5** — L'établissement désigne qui reçoit les alertes : au moins
  l'enseignant responsable de la séance, et éventuellement d'autres
  personnels (vie scolaire, infirmerie). Par défaut, l'enseignant
  responsable.
- **SENS-6** — Une alerte invite à une attention humaine ; elle ne
  déclenche aucune action automatique sur le poste.
- **SENS-7** — L'activation du module, ses destinataires et toute
  conservation éventuelle relèvent du chapitre « Données et RGPD » et
  supposent une analyse d'impact (AIPD).

### 5.5 IA générative

- **GEN-1** — L'IA générative sert à : proposer un modèle de TP à partir
  d'un cours ou d'une description (MODELE-5), proposer des sondages
  (SONDAGE-7), et aider un élève sur une étape (action « aide de l'IA »,
  MODELE-2).
- **GEN-2** — Tout contenu proposé par l'IA est présenté à l'enseignant
  ou à l'élève comme une suggestion, et peut être modifié ou écarté.
  Rien n'est lancé ni affiché sans relecture (IA-5).
- **GEN-3** — Ce qui est envoyé au fournisseur est réduit au nécessaire.
  Quand une analyse porte sur l'écran ou le travail d'un élève,
  l'enseignant est averti qu'une capture peut contenir des données
  personnelles, et choisit en conséquence le fournisseur (5.6).
- **GEN-4** — L'aide de l'IA à un élève (MODELE-2) affiche un conseil sur
  son poste ; elle ne transmet pas à l'enseignant le contenu analysé,
  mais peut lui signaler qu'une aide a été proposée.

### 5.6 Fournisseur d'IA

- **FOURN-1** — L'enseignant choisit le fournisseur d'IA générative à un
  seul endroit, côté console ou relais. Les postes élèves n'en
  connaissent aucun.
- **FOURN-2** — Fournisseurs possibles : un modèle **local** (sur le
  relais ou le poste de l'enseignant), un service **hébergé en France**,
  ou un service tiers (par exemple Mistral, Google). L'établissement
  peut restreindre la liste.
- **FOURN-3** — Une seule clé d'accès éventuelle est détenue côté
  enseignant ou établissement. Aucune clé de fournisseur n'est déployée
  sur les postes élèves (correction d'un défaut de la version
  précédente).
- **FOURN-4** — En l'absence d'Internet, seul un fournisseur local est
  utilisable ; les fonctions génératives sont sinon indisponibles, sans
  empêcher le reste de fonctionner (IA-1).
- **FOURN-5** — Pour les traitements touchant à des données d'élèves,
  l'établissement peut imposer un fournisseur local ou hébergé en
  France.

### 5.7 Poste personnel

- **IA-6** — Sur un poste personnel, le module de détection de
  harcèlement et de détresse ne s'applique pas (cohérent avec
  « observer et signaler seulement », chapitre 1). Les fonctions
  génératives d'aide pédagogique restent disponibles comme au chapitre
  3.

### 5.8 Critères d'acceptation

1. Surveo fonctionne entièrement avec toutes les fonctions d'IA
   désactivées.
2. Un classifieur d'activité signale un jeu dans la console sans
   qu'aucune capture ni frappe ne quitte le poste.
3. Le module de détection de harcèlement et de détresse est inactif tant
   que le rôle établissement ne l'a pas activé.
4. Une fois le module activé, une situation préoccupante produit une
   alerte portant un niveau et un motif général, sans transmettre le
   texte saisi ; l'alerte parvient aux destinataires désignés.
5. Pendant un examen, aucune analyse par l'IA n'a lieu.
6. Un TP proposé par l'IA s'ouvre en modification et n'est lancé
   qu'après validation de l'enseignant.
7. Aucune clé de fournisseur d'IA n'est présente sur un poste élève.
8. Sans Internet, les fonctions génératives sont indisponibles si aucun
   fournisseur local n'est configuré, mais le reste de Surveo marche.

### 5.9 Décisions

29 octobre 2026 : chapitre proposé, à valider.

- Deux familles distinctes : classifieurs locaux sur le poste, IA
  générative par le relais.
- Changement majeur par rapport à la version précédente : les
  classifieurs tournent sur le poste élève ; les données analysées (dont
  les frappes) n'en sortent pas ; seule une étiquette ou une alerte
  remonte. Aucun contenu frappé n'est transmis ni conservé.
- Détection de harcèlement et de détresse traitée en module sensible :
  désactivé par défaut, activé par l'établissement, analyse locale,
  destinataires désignés, AIPD.
- Fournisseur d'IA générative interchangeable, choisi côté enseignant ;
  aucune clé déployée sur les postes élèves ; fournisseur local ou
  hébergé en France imposable pour les données d'élèves.
- Aucune analyse par l'IA pendant un examen.

30 octobre 2026, après le prototype `classifieur` (IMPL.md, 5.1) :

- Les classifieurs sont embarqués sur le poste au format ONNX quantifié,
  sans recourir à un modèle distillé. Celui de harcèlement et de détresse
  occupe environ 140 Mo sur disque et 190 à 230 Mo de mémoire quand il
  est actif ; il analyse une phrase en 10 à 15 ms, sous Windows comme
  sous Linux.
- Le modèle n'est pas livré avec l'agent : il est téléchargé depuis le
  relais quand l'établissement active le module, et n'est chargé que
  tant que le module est actif (SENS-2).

### 5.10 Questions ouvertes

- Un antivirus peut signaler un agent qui lit le clavier : à vérifier
  tôt ; le fait que rien ne soit exfiltré doit faciliter l'acceptation.
- Niveaux d'alerte de détresse : combien, et quel libellé pour éviter
  les faux positifs anxiogènes ?
- Les classifieurs publiés avec l'agent (open source) sont extractibles :
  conséquence assumée, la valeur passant aux mises à jour des modèles et
  au service (voir VISION.md).
- Faut-il journaliser le fait qu'une alerte sensible a été émise (sans
  son contenu), pour la traçabilité, ou ne rien garder du tout
  (chapitre 8) ?

---

## 6. Rôles et confiance

Ce chapitre dit qui a le droit de faire quoi, et comment un poste décide
à qui obéir, sans qu'aucune inscription ni connexion à Internet ne soit
nécessaire.

### 6.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Rôle** | Ensemble de droits. Une personne (via sa console) peut cumuler plusieurs rôles. |
| **Établissement** | Regroupement des salles, des postes et des consoles qui se font mutuellement confiance. Au sens de Surveo, cela peut être un lycée, un centre de formation, ou simplement « la configuration d'un enseignant autonome ». |
| **Clé d'établissement** | Secret propre à l'établissement, créé localement à sa mise en place. C'est elle qui fonde la confiance ; elle ne quitte jamais l'établissement et n'est pas commune à tous les clients de Surveo. |
| **Console autorisée** | Console dont l'identité a été reconnue dans l'établissement, avec un ou plusieurs rôles. |
| **Appairage** | Opération par laquelle une nouvelle console devient autorisée. |
| **Code d'appairage** | Code court et temporaire qui permet d'autoriser une console. |

### 6.2 Les trois rôles

- **ROLE-1** — Surveo définit trois rôles :
  - **Enseignant** : conduire des séances (chapitre 1), superviser,
    appliquer les restrictions de séance (chapitre 2), mener les
    activités pédagogiques (chapitre 3) et communiquer (chapitre 4).
  - **Référent de salle / technicien** : gérer les plans de salle
    (chapitre 1), déployer et mettre à jour les agents (chapitre 7),
    appairer les consoles (6.5), et utiliser les outils techniques bruts
    (console de commandes, gestionnaire de fichiers, liste des
    processus).
  - **Établissement** : activer les modules sensibles (chapitre 5),
    régler la conservation des données et lire le journal d'audit
    (chapitre 8), définir les listes communes (sites, applications
    interdites).
- **ROLE-2** — Une même personne peut cumuler les trois rôles. C'est le
  cas courant d'un enseignant qui est aussi le référent de sa salle.
- **ROLE-3** — Une console n'expose que les fonctions des rôles qu'elle
  détient. Les autres ne sont pas affichées.
- **ROLE-4** — Les outils techniques bruts (console de commandes,
  fichiers, processus) relèvent du seul rôle technicien. L'enseignant
  dispose à la place des actions cadrées des chapitres 2 à 4 (par
  exemple fermer une application plutôt que parcourir les processus).
- **ROLE-5** — Les actions des rôles technicien et établissement sont
  consignées au journal d'audit (6.7).

### 6.3 Confiance sans inscription

> **Un poste n'obéit qu'aux consoles et relais autorisés dans son
> établissement. Il n'y a ni compte en ligne, ni secret commun à toutes
> les installations de Surveo.**

- **CONF-1** — La clé d'établissement est créée localement à la mise en
  place, sans Internet. Deux manières de démarrer une console :
  - **créer un établissement** : la console en devient le premier
    membre et détient d'office les trois rôles ;
  - **rejoindre un établissement existant** au moyen d'un code
    d'appairage (6.5).
- **CONF-2** — Un poste est rattaché à un établissement (et, pour un
  poste de salle, à une salle) lors de son installation (chapitre 7).
- **CONF-3** — Un poste n'applique une restriction ou une action que si
  elle provient d'une console autorisée de son établissement, détenant
  le rôle correspondant. Toute autre demande est ignorée.
- **CONF-4** — Le relais ne relaie qu'entre membres d'un même
  établissement. Un relais hébergé peut servir plusieurs établissements
  sans jamais mêler leurs flux.
- **CONF-5** — La compromission d'une console n'expose que son
  établissement, jamais les autres.

### 6.4 Un enseignant autonome

- **AUTO-1** — Un enseignant seul peut installer la console et des agents
  et s'en servir immédiatement, sans compte et sans Internet, en
  « créant un établissement » qui n'est que sa propre configuration
  (CONF-1).
- **AUTO-2** — Il détient alors les trois rôles (CONF-1) et peut, plus
  tard, rejoindre ou fusionner avec l'établissement de son lycée si
  besoin (question ouverte, 6.10).

### 6.5 Appairer une console

- **PAIR-1** — Pour autoriser une nouvelle console, un détenteur du rôle
  établissement ou technicien engendre un code d'appairage, court et à
  durée de vie limitée.
- **PAIR-2** — La nouvelle console saisit ce code et devient autorisée,
  avec le ou les rôles que le code accorde.
- **PAIR-3** — Par défaut, un code n'accorde que le rôle enseignant. Les
  rôles technicien et établissement doivent être accordés
  explicitement.
- **PAIR-4** — L'appairage se fait par l'intermédiaire du relais de
  l'établissement : il fonctionne hors ligne tant que le relais est
  joignable localement.
- **PAIR-5** — Un détenteur du rôle établissement peut retirer
  l'autorisation d'une console (départ d'un collègue, poste perdu).
  Celle-ci n'est alors plus obéie par les postes (CONF-3).
- **PAIR-6** — Les appairages et retraits sont consignés au journal
  d'audit (6.7).

### 6.6 Plusieurs consoles sur une séance

- **MULTI-1** — Conformément à SEANCE-2, plusieurs consoles autorisées
  peuvent se joindre à une séance ouverte.
- **MULTI-2** — La séance a un **responsable** : l'enseignant qui l'a
  ouverte. Lui seul mène les activités pédagogiques et clôt la séance.
- **MULTI-3** — Les autres consoles agissent dans la limite de leurs
  rôles : une console technicien observe et peut intervenir
  techniquement, une autre console enseignant peut observer et
  communiquer. La console responsable voit qui est joint à la séance.
- **MULTI-4** — Tout ce que fait une console sur la séance reste soumis à
  la transparence (MAIT-1) et au journal d'audit pour les rôles
  technicien et établissement.

### 6.7 Journal d'audit

- **AUDIT-1** — Surveo tient un journal des actions sensibles : appairage
  et retrait de consoles (6.5), restrictions appliquées (MAIT-4),
  activation des modules sensibles et actions techniques (console de
  commandes, fichiers, processus).
- **AUDIT-2** — Chaque entrée indique qui, quoi, quand, et sur quel
  poste.
- **AUDIT-3** — Le journal d'audit est lisible par le rôle
  établissement. Sa conservation et son éventuelle purge relèvent du
  chapitre « Données et RGPD ».
- **AUDIT-4** — Le journal d'audit protège aussi l'enseignant et le
  technicien : il atteste de ce qui a été fait, et par qui.

### 6.8 Critères d'acceptation

1. Une console qui ne détient que le rôle enseignant n'affiche ni la
   console de commandes, ni le gestionnaire de fichiers, ni la liste des
   processus.
2. Un enseignant installe la console et un agent, sans Internet et sans
   compte, et peut aussitôt ouvrir une séance : il détient les trois
   rôles.
3. Un poste ignore une demande d'action émanant d'une console d'un autre
   établissement.
4. Un collègue devient enseignant en saisissant un code d'appairage ; ce
   code ne lui donne ni le rôle technicien ni le rôle établissement.
5. Après le retrait de l'autorisation d'une console, les postes
   n'exécutent plus ses demandes.
6. Une action technique (ouverture de la console de commandes sur un
   poste) apparaît au journal d'audit avec son auteur et son horodatage.
7. Deux consoles sont jointes à une même séance ; seule la console
   responsable peut lancer un TP et clôturer la séance.

### 6.9 Décisions

29 octobre 2026 : chapitre proposé, à valider.

- Trois rôles cumulables : enseignant, référent de salle / technicien,
  établissement. Une même personne les cumule souvent.
- Confiance fondée sur une clé d'établissement locale, sans inscription
  ni secret commun à toutes les installations (contrairement à la
  version précédente, où une clé unique était partagée par tous les
  clients).
- Un enseignant autonome crée son propre établissement et détient les
  trois rôles.
- Appairage d'une console par code court et temporaire, via le relais,
  hors ligne ; retrait possible par le rôle établissement.
- Outils techniques bruts réservés au rôle technicien ; l'enseignant
  dispose des actions cadrées des chapitres 2 à 4.
- Une séance a un responsable (celui qui l'ouvre) ; les autres consoles
  agissent dans la limite de leurs rôles.
- Journal d'audit des actions sensibles, lisible par le rôle
  établissement.

### 6.10 Questions ouvertes

- Fusion de deux établissements : un enseignant autonome qui rejoint le
  dispositif de son lycée garde-t-il ses salles et ses séances ?
- Perte de la clé d'établissement : quelle procédure de récupération,
  sans introduire de porte dérobée ?
- Un relais hébergé doit-il pouvoir prouver à un établissement qu'il ne
  lit pas ses flux (chiffrement de bout en bout entre consoles et
  postes) ?
- Rôles plus fins au sein de l'établissement (chef d'établissement vs
  délégué à la protection des données) : utile en v1, ou plus tard ?
- Un élève a-t-il, lui aussi, une forme d'identité de confiance, ou
  reste-t-il entièrement du côté « poste » (chapitre 1) ?

---

## 7. Déploiement et relais

Ce chapitre dit comment on installe Surveo, comment un poste trouve le
relais de son établissement, et comment on reconfigure une salle sans
intervenir poste par poste.

### 7.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Agent** | Programme installé sur un poste élève. |
| **Console** | Application de l'enseignant. |
| **Relais** | Programme par lequel passent tous les échanges d'un établissement. Les agents et les consoles s'y connectent en sortie ; le relais n'a pas besoin de joindre les postes. |
| **Rattachement** | Lien qui associe un poste à un établissement, et, pour un poste de salle, à une salle. |
| **Configuration de poste** | Réglages d'un poste poussés par le relais : son relais, sa salle, son type, les listes communes. |
| **Paquet d'installation** | Fichier d'installation de l'agent, portant déjà le rattachement. |
| **Découverte** | Façon dont un poste trouve le relais à joindre. |

### 7.2 Installation des agents

- **INST-1** — L'agent s'installe selon l'un des deux modes de
  déploiement (MODE-1) :
  - **sans droits d'administration** : installé dans le compte de
    l'utilisateur, démarré au niveau de la session, sans service
    système ni pilote ;
  - **déployé par l'établissement** : installé par le service
    informatique, par ses moyens habituels (stratégie de groupe, image
    système, outil de gestion de parc).
- **INST-2** — À l'installation, un poste est rattaché à un
  établissement (CONF-2) et reçoit son type : **poste de salle** ou
  **poste personnel** (chapitre 1). Un poste de salle est aussi rattaché
  à une salle.
- **INST-3** — Le rattachement est porté par le paquet d'installation :
  un référent prépare un paquet pour une salle donnée, et chaque poste
  installé avec ce paquet rejoint l'établissement et la salle sans
  saisie manuelle.
- **INST-4** — Un poste personnel s'installe avec un paquet, un code de
  séance ou un lien fourni par l'enseignant, sans droits
  d'administration.
- **INST-5** — Désinstaller l'agent remet le poste dans son état
  d'origine (RESTR-4) et le détache de l'établissement.
- **INST-6** — L'agent ne réclame aucun droit qu'il n'utilise pas. En
  mode sans administration, il n'obtient aucune capacité réservée à
  l'administrateur.

### 7.3 Le relais

- **RELAIS-1** — Un même relais peut être installé à trois endroits,
  selon ce qu'autorise l'établissement :
  - **dans la classe**, au sein de la console de l'enseignant, pour une
    salle autonome sans aucun serveur ;
  - **sur un serveur de l'établissement** ;
  - **sur un serveur hébergé** (en France pour l'offre hébergée).
- **RELAIS-2** — Le relais est le même programme dans les trois cas ;
  seul son emplacement change.
- **RELAIS-3** — Un relais ne relaie qu'entre membres d'un même
  établissement (CONF-4). Un relais hébergé peut servir plusieurs
  établissements sans mêler leurs flux.
- **RELAIS-4** — Agents et consoles se connectent **en sortie** vers le
  relais. Aucun poste élève n'accepte de connexion entrante ni n'écoute
  le réseau, ce qui lève les obstacles des réseaux séparés et des
  pare-feux.
- **RELAIS-5** — Une séance à distance (REJ-5) exige un relais joignable
  depuis Internet ; une salle autonome fonctionne avec un relais
  purement local, sans Internet.
- **RELAIS-6** — Un relais installé dans la classe reçoit des
  connexions. Sur un poste Windows, cela demande une autorisation du
  pare-feu, accordée une fois par un administrateur ou par le service
  informatique. Quand elle manque, la console le signale à l'enseignant
  et propose un relais d'établissement ou hébergé.

### 7.4 Découverte du relais

- **DECOUV-1** — Un poste trouve son relais en essayant, dans l'ordre :
  1. le relais qu'il connaît déjà (dernière configuration reçue) ;
  2. une indication publiée sur le réseau de l'établissement (par
     exemple un enregistrement de service), si le service informatique
     l'a prévue ;
  3. une recherche émise par le poste sur le réseau local, à laquelle
     répond la console de l'enseignant présente dans la salle ;
  4. en dernier recours, l'annuaire de l'offre hébergée.
- **DECOUV-2** — La découverte ne contourne jamais la confiance : un
  poste qui trouve un relais ne lui obéit que s'il appartient à son
  établissement (CONF-3).
- **DECOUV-3** — Un poste personnel n'utilise que le relais indiqué par
  le code ou le lien de séance (INST-4) : il ne cherche pas de relais sur
  un réseau local.

### 7.5 Reconfiguration d'une salle

> **On ne reconfigure jamais une salle poste par poste : une
> modification se décide une fois et se propage.**

- **RECONF-1** — Le relais pousse une configuration de poste aux agents
  connectés. La configuration est signée au nom de l'établissement ; un
  agent n'applique qu'une configuration dont la signature est reconnue
  (CONF-3).
- **RECONF-2** — Pour changer de relais (par exemple passer d'un relais
  hébergé à un relais de classe), le référent le décide une fois depuis
  la console. La nouvelle adresse est poussée aux postes connectés, qui
  basculent ensuite sur le nouveau relais.
- **RECONF-3** — Un poste éteint ou absent au moment du changement
  retrouve le bon relais à son prochain démarrage grâce à la chaîne de
  découverte (7.4).
- **RECONF-4** — La modification du plan de salle, du type d'un poste ou
  des listes communes passe aussi par une configuration poussée, sans
  réinstallation.
- **RECONF-5** — Les reconfigurations sont consignées au journal d'audit
  (AUDIT-1).

### 7.6 Mises à jour

- **MAJ-1** — Les agents et les consoles se mettent à jour par le relais,
  sans intervention sur chaque poste.
- **MAJ-2** — Une mise à jour n'est installée que si elle est signée par
  le projet Surveo. Un relais, même compromis, ne peut pas pousser du
  code non signé.
- **MAJ-3** — Une mise à jour en mode sans administration s'installe dans
  le compte de l'utilisateur, sans droits d'administrateur.
- **MAJ-4** — Une mise à jour ne s'applique pas pendant une séance en
  cours sur le poste : elle attend la fin de la séance ou le prochain
  démarrage, pour ne pas interrompre le travail.
- **MAJ-5** — L'établissement peut retarder ou figer les versions, pour
  rester maître de son parc.

### 7.7 Démarrage et état propre

- **DEMAR-1** — Au démarrage, l'agent se lance au niveau de la session
  (mode sans administration) ou au niveau du système (déployé par
  l'établissement), puis cherche son relais (7.4).
- **DEMAR-2** — Conformément à RESTR-3, l'agent remet d'abord le poste
  dans son état d'origine, avant toute connexion ; il ne réapplique des
  restrictions que s'il rejoint une séance en cours.
- **DEMAR-3** — Un poste de salle qui ne trouve aucune séance ouverte
  reste en veille et apparaît *allumé* dans la console (SALLE-1).

### 7.8 Critères d'acceptation

1. Un enseignant sans droits d'administration installe l'agent sur un
   poste élève et la console sur son poste, et supervise la salle le jour
   même.
2. Un poste installé avec le paquet d'une salle rejoint l'établissement
   et la salle sans aucune saisie manuelle.
3. Dans une salle autonome, sans serveur ni Internet, les postes
   trouvent le relais intégré à la console de l'enseignant et rejoignent
   la séance.
4. Le référent change le relais d'une salle depuis la console ; les
   postes allumés basculent sans être touchés un par un, et un poste qui
   était éteint rejoint le bon relais à son démarrage suivant.
5. Un relais hébergé sert deux établissements : aucun poste de l'un ne
   voit un flux de l'autre.
6. Une mise à jour d'agent non signée, poussée par un relais, est
   refusée par les postes.
7. Une mise à jour disponible pendant une séance ne s'installe qu'après
   la fin de la séance.
8. Après désinstallation, le poste ne porte plus aucune trace de Surveo
   et ses réglages sont ceux d'origine.

### 7.9 Décisions

29 octobre 2026 : chapitre proposé, à valider.

- Architecture inversée par rapport à la version précédente : les agents
  et consoles se connectent en sortie vers un relais, au lieu que la
  console ouvre des ports entrants sur chaque poste.
- Relais unique, installable dans la classe, sur un serveur de
  l'établissement ou sur un serveur hébergé.
- Installation possible sans droits d'administration (démarrage au
  niveau de la session) ; déploiement par l'établissement également
  possible.
- Rattachement porté par le paquet d'installation (établissement,
  salle, type de poste).
- Chaîne de découverte à quatre niveaux pour trouver le relais sans
  configuration manuelle.
- Reconfiguration par configuration signée poussée depuis le relais : on
  ne touche jamais les postes un par un.
- Mises à jour signées par le projet Surveo (contrairement à la version
  précédente, où du code était poussé sans signature avec une clé
  partagée) ; pas de mise à jour pendant une séance.
- 29 octobre 2026, après le prototype `tauri` (IMPL.md, 7.2) :
  - DECOUV-1 et RELAIS-4 : la découverte locale part du poste et le
    relais répond ; un poste élève n'écoute jamais le réseau (le
    pare-feu de Windows réagit sinon).
  - RELAIS-6 : un relais dans la classe demande une autorisation du
    pare-feu, accordée une fois par un administrateur ; à défaut, relais
    d'établissement ou hébergé.

### 7.10 Questions ouvertes

- Préparation des paquets : outil dédié dans la console (rôle
  technicien), ou procédure documentée pour le service informatique ?
- Découverte sur le réseau de l'établissement : quel mécanisme exact
  proposer aux services informatiques (enregistrement de service,
  réglage poussé par stratégie de groupe) ?
- Bascule de relais : faut-il une période où le poste accepte
  l'ancien et le nouveau relais, pour les postes à connexion
  intermittente ?
- Vérification des mises à jour : builds reproductibles et signatures
  publiques, dans l'esprit open source ?
- Un même poste physique peut-il porter deux rattachements (deux
  établissements partageant une salle) ?

---

## 8. Données et RGPD

Ce chapitre rassemble les règles sur les données : ce que Surveo
collecte, où cela se trouve, combien de temps, et qui en répond. Il
donne corps au principe de minimisation présent dans toute la
spécification.

### 8.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Donnée personnelle** | Toute information se rapportant à une personne identifiée ou identifiable (un élève, un enseignant). |
| **Responsable de traitement** | Celui qui décide des traitements et en répond : l'établissement. |
| **Sous-traitant** | Celui qui traite des données pour le compte du responsable : l'exploitant d'un relais hébergé. |
| **Minimisation** | Ne collecter que ce qui est nécessaire, pas davantage. |
| **Conservation** | Durée pendant laquelle une donnée est gardée. |
| **Traçabilité** | Historique des séances et des travaux, conservé sur option (8.5). |
| **AIPD** | Analyse d'impact relative à la protection des données. |

### 8.2 Principes

> **Par défaut, rien ne subsiste après la séance. Ce qui est conservé
> l'est parce que l'établissement l'a explicitement choisi, pour une
> durée qu'il fixe.**

- **RGPD-1** — Surveo collecte le minimum nécessaire à chaque fonction
  (minimisation).
- **RGPD-2** — L'établissement est responsable de traitement. Il décide
  des modules activés (chapitres 5 et 6), des conservations (8.4) et des
  destinataires des alertes (SENS-5).
- **RGPD-3** — Les flux en direct (écrans, étiquettes d'activité,
  conversations) sont éphémères : ils ne sont pas enregistrés, sauf
  décision de traçabilité (8.5).
- **RGPD-4** — Les traitements sont transparents pour les élèves :
  indicateur de séance permanent (REJ-4), information renforcée pour les
  modules sensibles (SENS-4).
- **RGPD-5** — S'agissant pour l'essentiel de mineurs, Surveo retient par
  défaut l'option la plus protectrice à chaque choix.

### 8.3 Quelles données, où

- **DATA-1** — Inventaire des données manipulées par Surveo :

  | Donnée | Emplacement | Conservation par défaut |
  |---|---|---|
  | Écran en direct, étiquettes d'activité | En transit, affiché à la console | Non conservé |
  | Conversations, messages, demandes d'aide | En transit via le relais | Non conservé |
  | Rendus de TP | Côté enseignant / établissement | Conservé (8.4) |
  | Configuration (salles, plans, listes, rattachements) | Relais / établissement | Conservé tant qu'utile |
  | Journal d'audit | Relais / établissement | Selon réglage (8.6) |
  | Alertes de modules sensibles | Vers les destinataires désignés | Non conservé par défaut (8.7) |
  | Contenu frappé au clavier | Sur le poste élève uniquement | Jamais transmis ni conservé (IA-2, SENS-3) |

- **DATA-2** — En déploiement local ou sur un serveur de l'établissement,
  aucune donnée ne quitte l'établissement.
- **DATA-3** — Avec un relais hébergé, les seules données qui transitent
  hors de l'établissement sont celles nécessaires au relais ; leur
  localisation suit 8.8.

### 8.4 Conservation

- **CONSERV-1** — Par défaut, seules deux catégories subsistent après une
  séance : les **rendus de TP** et la **configuration**. Tout le reste
  est éphémère (RGPD-3).
- **CONSERV-2** — L'établissement fixe les durées de conservation des
  rendus, du journal d'audit et, le cas échéant, de la traçabilité.
  Surveo propose des durées par défaut prudentes.
- **CONSERV-3** — À l'échéance d'une durée de conservation, la donnée est
  supprimée automatiquement.
- **CONSERV-4** — L'enseignant peut supprimer les rendus d'une séance ;
  l'établissement peut supprimer toute donnée dont il répond.

### 8.5 Traçabilité

- **TRACE-1** — La traçabilité est une **option de l'établissement,
  désactivée par défaut**. Activée, elle conserve un historique des
  séances : quand, quelle salle, quel enseignant, quels TP, quels
  rendus, quels sondages.
- **TRACE-2** — Elle sert surtout aux centres de formation (preuve de
  présence, de rendu, suivi d'acquisition). Elle n'est pas nécessaire à
  l'usage courant d'un enseignant.
- **TRACE-3** — L'historique peut être rattaché à un élève lorsque son
  identité a été renseignée (chapitre 1) ; sinon il reste au niveau du
  poste et de la séance.
- **TRACE-4** — Sa durée de conservation est réglée par l'établissement
  (CONSERV-2) et l'historique est exportable.

### 8.6 Journal d'audit

- **AUDITC-1** — Le journal d'audit (chapitre 6) conserve les actions
  sensibles pour la durée fixée par l'établissement.
- **AUDITC-2** — Il ne contient pas le contenu des échanges ni des
  travaux : seulement qui a fait quoi, quand, sur quel poste.
- **AUDITC-3** — Il est lisible par le rôle établissement et exportable.

### 8.7 Modules sensibles

- **SENSC-1** — Les alertes du module de détection de harcèlement et de
  détresse (chapitre 5) ne sont **pas conservées par défaut** : elles
  sont transmises aux destinataires, puis oubliées par Surveo.
- **SENSC-2** — L'établissement peut choisir de journaliser le fait
  qu'une alerte a été émise (horodatage, poste, niveau), sans son
  contenu, s'il le juge utile et l'a inscrit à son AIPD.
- **SENSC-3** — L'activation d'un module sensible requiert une AIPD
  (SENS-7). Surveo fournit un modèle d'AIPD (8.9).

### 8.8 Hébergement et localisation

- **HEBERG-1** — En relais local ou d'établissement, les données restent
  sous le seul contrôle de l'établissement.
- **HEBERG-2** — Le relais hébergé est situé en France. L'exploitant du
  relais hébergé agit comme sous-traitant et un contrat de
  sous-traitance encadre son intervention.
- **HEBERG-3** — Surveo cherche à ce qu'un relais, y compris hébergé, en
  voie le moins possible : le chiffrement de bout en bout entre consoles
  et postes est un objectif (question ouverte, 6.10).

### 8.9 Droits des personnes et kit RGPD

- **DROITS-1** — Les demandes des personnes (information, accès,
  effacement) sont adressées à l'établissement, responsable de
  traitement. Surveo lui donne les moyens d'y répondre (retrouver et
  supprimer les données d'un élève, exporter la traçabilité).
- **DROITS-2** — Surveo fournit un **kit RGPD** : fiche pour le registre
  des traitements, modèle d'AIPD pour les modules sensibles, et modèle
  de contrat de sous-traitance pour l'offre hébergée.
- **DROITS-3** — Le kit et la documentation sont en français (voir
  VISION.md, crédibilité).

### 8.10 Critères d'acceptation

1. Après une séance ordinaire, sans traçabilité, il ne reste des données
   que les rendus et la configuration ; les écrans, conversations et
   étiquettes d'activité n'ont pas été enregistrés.
2. En déploiement local, aucune donnée ne sort de l'établissement
   pendant une séance complète.
3. À l'échéance de la durée de conservation fixée, les rendus concernés
   sont supprimés automatiquement.
4. Traçabilité désactivée, aucun historique de séance n'est conservé ;
   activée, l'historique d'une séance est présent et exportable.
5. Une alerte de détresse est transmise à ses destinataires sans être
   conservée, sauf si l'établissement a choisi d'en journaliser
   l'émission, et alors sans son contenu.
6. Le contenu frappé au clavier n'apparaît dans aucune donnée conservée
   ni transmise.
7. L'établissement peut retrouver et supprimer les données rattachées à
   un élève nommé.

### 8.11 Décisions

29 octobre 2026 : chapitre proposé, à valider.

- Minimisation : par défaut, rien ne subsiste après la séance, hormis
  rendus et configuration.
- L'établissement est responsable de traitement ; l'exploitant d'un
  relais hébergé est sous-traitant, encadré par contrat, en France.
- Flux en direct éphémères ; contenu frappé jamais transmis ni conservé.
- Traçabilité en option, désactivée par défaut, pensée pour les centres
  de formation.
- Alertes sensibles non conservées par défaut ; journalisation
  éventuelle sans contenu, sous AIPD.
- Durées de conservation réglées par l'établissement, suppression
  automatique à l'échéance.
- Kit RGPD fourni (registre, AIPD, contrat de sous-traitance), en
  français.

### 8.12 Questions ouvertes

- Durées de conservation par défaut à proposer (rendus, journal d'audit,
  traçabilité).
- Portée exacte du chiffrement de bout en bout (lié à 6.10) : quelles
  données le relais doit-il pouvoir voir pour fonctionner ?
- Consentement et information : quel partage entre la charte
  informatique de l'établissement et un message affiché par Surveo ?
- Élèves mineurs : faut-il des garde-fous supplémentaires sur la
  traçabilité nominative (durée plafonnée, accès restreint) ?
- Export des données d'un élève : format et périmètre pour répondre à
  une demande d'accès.

---

## 9. Interface et ergonomie

Ce chapitre traduit l'exigence d'une « ergonomie soignée » en principes,
en structure et en critères vérifiables. Il complète les vues de la
salle définies au chapitre 1 (AFF-1 à AFF-4). La charte visuelle
(couleurs, typographie, composants) relève d'IMPL.md.

### 9.1 Vocabulaire

| Terme | Sens |
|---|---|
| **Espace** | Partie de la console consacrée à une tâche : préparer, conduire la séance, administrer. |
| **Vue** | Manière de regarder la salle dans l'espace Séance : plan, liste, mosaïque, focus (AFF-3). |
| **Sélection** | Ensemble des postes sur lesquels portera la prochaine action. |
| **Barre d'actions** | Barre qui propose les actions applicables à la sélection. |
| **Panneau** | Zone latérale de la console où vivent les activités en cours (aide, TP, sondage, conversations). |
| **Recherche d'actions** | Champ, ouvert au clavier, où l'on tape le nom d'une action ou d'un poste. |
| **Retour d'action** | Compte rendu, poste par poste, de ce qu'une action a produit. |
| **Mode projection** | Mode de la console pensé pour un écran projeté devant la classe, où rien de sensible ne s'affiche. |
| **Panneau élève** | Point d'entrée unique de l'élève vers Surveo sur son poste. |

### 9.2 Principes

> **L'interface s'organise par tâche, pas par fonctionnalité. L'état de
> la salle se lit d'un coup d'œil, et rien ne s'impose à l'écran sans
> nécessité, ni chez l'enseignant ni chez l'élève.**

- **ERGO-1** — La console est organisée par tâche (préparer, conduire,
  administrer), et non par liste de fonctionnalités. Ajouter une
  fonction ne doit pas ajouter un bouton ou une fenêtre de plus par
  défaut.
- **ERGO-2** — Il n'y a qu'une console. Les rôles ajoutent des espaces
  et des actions (ROLE-3) ; aucun rôle n'a d'application distincte.
- **ERGO-3** — Divulgation progressive : les actions fréquentes sont
  visibles, les actions rares restent accessibles sous « Plus » ou par
  la recherche d'actions.
- **ERGO-4** — L'état de la salle se lit d'un coup d'œil : présence,
  demandes d'aide, retards, restrictions en cours. Les signaux sont
  hiérarchisés, du plus urgent au plus anodin, sans bruit permanent.
- **ERGO-5** — Une même action porte le même nom, la même icône et se
  trouve au même endroit partout. L'interface emploie le vocabulaire de
  cette spécification (poste, séance, salle, enseignant, élève).
- **ERGO-6** — Tous les textes sont en français, sans jargon technique.
  Un message dit ce qui se passe et, si besoin, ce que l'on peut faire.
- **ERGO-7** — Une notification discrète est préférée à une fenêtre
  qui interrompt, sauf quand l'interruption est le but même de
  l'action (écran en pause, message, sondage).

### 9.3 Espaces

- **ESPACE-1** — La console comporte trois espaces :
  - **Préparer** : la bibliothèque de l'enseignant (modèles de TP,
    sondages, messages prédéfinis, listes de sites et d'applications,
    listes de classe) ;
  - **Séance** : la salle, ses vues, la barre d'actions et le panneau ;
  - **Administrer** : salles et plans, postes et déploiement, appairage,
    outils techniques, modules sensibles, conservation, journal d'audit.
- **ESPACE-2** — Séance est l'espace affiché à l'ouverture de la
  console.
- **ESPACE-3** — L'espace Administrer n'apparaît que pour une console
  détenant le rôle technicien ou établissement (ROLE-3). Son contenu
  suit les rôles détenus.
- **ESPACE-4** — Changer d'espace ne ferme pas la séance. Tant qu'une
  séance est ouverte, un bandeau la rappelle dans chaque espace et
  permet d'y revenir d'un geste.

### 9.4 Conduire la séance

- **SEL-1** — Le principe est de **sélectionner, puis agir**.
  L'enseignant sélectionne un ou plusieurs postes (clic, rectangle de
  sélection, tout sélectionner, ou critère depuis la vue liste : « ceux
  qui n'ont pas rendu »). La barre d'actions propose alors les actions
  applicables.
- **SEL-2** — Sans sélection, une action porte sur toute la séance, et
  la barre l'indique explicitement :

  > Toute la salle (24 postes)

- **SEL-3** — La barre d'actions montre directement les actions
  courantes (écran en pause, message, fichier, TP, sondage) et range les
  autres sous « Plus ».
- **SEL-4** — Quand une action ne s'applique qu'à une partie de la
  sélection, la barre le dit au lieu de l'ignorer en silence :

  > S'applique à 20 postes sur 24 : 4 postes personnels ne sont pas
  > concernés.

- **SEL-5** — La tuile d'un poste offre un menu court : les actions de
  la barre, plus celles propres à un poste (identifier, focus,
  conversation).
- **PANNEAU-1** — Les activités en cours (demandes d'aide, TP, sondage,
  diffusion, conversations) vivent dans le panneau latéral, pas dans des
  fenêtres séparées.
- **PANNEAU-2** — Le panneau se replie ; chaque activité garde alors un
  compteur visible (« Aide (2) »).
- **PANNEAU-3** — Pendant une séance, l'espace Séance n'ouvre aucune
  fenêtre séparée. Lancer un TP ou un sondage se fait en choisissant
  dans la bibliothèque sans quitter la séance ; créer ou modifier un
  modèle se fait dans l'espace Préparer.
- **RECH-1** — La recherche d'actions s'ouvre par un raccourci clavier.
  On y tape le nom d'une action (« pause », « distribuer ») ou d'un
  poste pour l'exécuter ou l'atteindre.
- **RECH-2** — Les actions courantes ont un raccourci clavier, indiqué
  dans leur infobulle.

### 9.5 Retours et sûreté des actions

- **RETOUR-1** — Toute action sur plusieurs postes donne un retour par
  poste :

  > Envoyé à 23 postes · 1 injoignable (poste 07).

- **RETOUR-2** — Une action en attente, parce qu'un poste est
  momentanément déconnecté, apparaît comme telle jusqu'à son aboutissement
  (DISTRIB-4, MSG-5).
- **RETOUR-3** — Les actions réversibles (écran en pause, filtrage,
  fermeture du panneau…) s'appliquent sans confirmation préalable et
  proposent « Annuler » pendant quelques secondes.
- **RETOUR-4** — Seules les actions irréversibles ou de grande portée
  demandent une confirmation : clôturer la séance, remplacer un TP
  (TP-4), retirer une console (PAIR-5), actions techniques. La
  confirmation énonce la portée :

  > Clôturer la séance ? Les 24 postes reviendront à leur état
  > d'origine.

- **RETOUR-5** — Une erreur est expliquée en français, avec ce que
  l'enseignant peut faire ensuite.

### 9.6 Mode projection

- **PROJ-1** — L'enseignant active ou désactive le mode projection d'un
  geste, à tout moment.
- **PROJ-2** — En mode projection :
  - les images des écrans d'élèves sont masquées, sauf diffusion
    volontaire (DIFF-1, DIFF-3) ;
  - le contenu des conversations est masqué ;
  - les alertes des modules sensibles n'apparaissent pas : seul un
    indicateur neutre signale qu'une notification attend.
- **PROJ-3** — La console montre en permanence à l'enseignant qu'il est
  en mode projection.
- **PROJ-4** — Quel que soit le mode, une alerte d'un module sensible
  (chapitre 5) ne s'affiche jamais en grand ni dans une fenêtre
  surgissante : elle passe par un indicateur discret que l'enseignant
  ouvre lui-même.

### 9.7 Interface côté élève

- **AGENT-1** — Hors séance, l'agent n'affiche aucune fenêtre. Sur un
  poste de salle, il se limite au plus à une icône discrète.
- **AGENT-2** — Pendant la séance, l'indicateur REJ-4 est compact. Il
  peut être réduit ou déplacé, mais jamais masqué entièrement.
- **AGENT-3** — L'élève accède à tout ce que Surveo lui propose par un
  **panneau élève** unique : TP, fichiers reçus, conversation, demande
  d'aide, sondage en cours. Surveo ne multiplie pas les fenêtres sur le
  poste.
- **AGENT-4** — Seuls s'imposent au premier plan chez l'élève : l'écran
  en pause (ECRAN-1), un message (MSG-2), un sondage (SONDAGE-2) et
  l'indication d'examen (EXAM-1). Tout le reste passe par une
  notification discrète (CHAT-4).
- **AGENT-5** — Les textes affichés à l'élève sont courts et simples,
  compréhensibles par un collégien.

### 9.8 Accessibilité et confort

- **ACCES-1** — La console et l'interface élève visent la conformité au
  RGAA (référentiel général d'amélioration de l'accessibilité).
- **ACCES-2** — Tout se fait au clavier seul.
- **ACCES-3** — Thèmes clair et sombre, contrastes suffisants, taille du
  texte réglable.
- **ACCES-4** — Une information n'est jamais portée par la couleur
  seule : l'état d'un poste s'accompagne toujours d'une icône ou d'un
  texte.
- **ACCES-5** — La console reste lisible et utilisable sur un portable,
  un grand écran, un vidéoprojecteur et un tableau interactif (cibles
  tactiles assez grandes).

### 9.9 Critères d'acceptation

1. Après avoir sélectionné des postes, l'enseignant met en pause,
   envoie un message, distribue un fichier, lance un TP ou un sondage en
   deux gestes au plus.
2. Pendant une séance complète (TP, sondage, demandes d'aide,
   conversation, diffusion), la console n'ouvre aucune fenêtre séparée.
3. Une console qui ne détient que le rôle enseignant n'affiche pas
   l'espace Administrer.
4. Une action sur 25 postes dont un est injoignable affiche un retour
   qui nomme le poste injoignable.
5. Une mise en pause faite par erreur est annulée par « Annuler », sans
   qu'aucune confirmation n'ait été demandée avant.
6. En mode projection, aucune image d'écran d'élève (hors diffusion
   volontaire), aucun contenu de conversation et aucune alerte sensible
   n'est visible.
7. Une alerte sensible n'ouvre jamais de fenêtre surgissante.
8. Toutes les actions de l'espace Séance se font au clavier seul.
9. L'état de chaque poste reste compréhensible quand l'écran est affiché
   en niveaux de gris.
10. Lors d'un test de prise en main, un enseignant qui découvre Surveo
    ouvre une séance, place ses postes et lance un TP sans
    documentation, dans une durée cible à fixer.

### 9.10 Décisions

29 octobre 2026 : chapitre proposé, puis structure validée sur une
maquette cliquable (console enseignant et poste élève, données
fictives). L'organisation est jugée claire, et l'accès à toute action
courante en deux gestes au plus est confirmé comme un point fort.

- L'ergonomie est spécifiée par des principes et des critères
  vérifiables ; la charte visuelle relève d'IMPL.md.
- Distinction entre espaces (la tâche), vues (la façon de regarder la
  salle) et rôles (les droits) ; une seule console.
- Trois espaces : Préparer, Séance, Administrer ; Séance par défaut.
- Conduite de séance par « sélectionner, puis agir », panneau latéral
  plutôt que fenêtres, recherche d'actions au clavier.
- « Annuler » préféré à la confirmation, sauf actions irréversibles ou
  de grande portée.
- Mode projection, et alertes sensibles jamais affichées en grand.
- Côté élève, un panneau unique et le moins d'interruptions possible.

### 9.11 Questions ouvertes

- Densité : la maquette montre beaucoup d'informations à la fois. À
  surveiller lors du test de prise en main (critère 10), en particulier
  sur un écran de portable.
- Le mode projection doit-il se proposer de lui-même quand un écran
  supplémentaire est branché ?
- Faut-il prévoir dès la v1 un usage tactile complet (tableau
  interactif, tablette) ?
- Durée cible du test de prise en main (critère 10).
- L'enseignant peut-il personnaliser la barre d'actions (ordre, actions
  favorites) ?
- Noms définitifs des espaces (« Préparer », « Séance »,
  « Administrer »).
