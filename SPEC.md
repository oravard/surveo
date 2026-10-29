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
5. IA — à venir
6. Rôles et confiance (appairage, droits) — à venir
7. Déploiement et relais (installation, découverte, reconfiguration) — à venir
8. Données et RGPD (traçabilité, conservation, modules sensibles) — à venir

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
| **Restriction** | Tout changement de comportement d'un poste demandé pendant une séance : écran bloqué, filtrage web, application fermée, etc. |
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
- Un poste de salle allumé attend la séance et le prof le voit avant
  d'ouvrir la séance.

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
- **WEB-2** — Quand un site est bloqué, l'élève voit une page claire qui
  l'explique, plutôt qu'une erreur technique :

  > Accès suspendu pendant la séance de {enseignant}.

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

### 2.9 Critères d'acceptation

1. L'enseignant met en pause l'écran d'une sélection de postes avec un
   message personnalisé : les postes concernés affichent ce message et
   ne répondent plus aux actions de l'élève ; les autres postes ne sont
   pas touchés.
2. L'enseignant active un filtrage en liste d'autorisation : un site
   permis s'ouvre, un autre site affiche la page d'accès suspendu.
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
