# Surveo — spécification

Ce document dit ce que fait Surveo, vu de l'utilisateur : règles,
messages, critères d'acceptation, décisions. Il est rangé par thème.
La réalisation est décrite dans IMPL.md.

Chaque règle porte un identifiant (`SEANCE-3`, `RESTR-1`…) pour que le
code et les tests puissent y renvoyer.

## Chapitres

1. [Séance et salle](#1-séance-et-salle)
2. Maîtrise de la classe (blocage, filtrage, applications, mode examen) — à venir
3. Pédagogie (TP, sondages, distribution et rendu de fichiers) — à venir
4. Communication (messages, chat, diffusion d'écran) — à venir
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
