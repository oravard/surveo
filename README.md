# Surveo

Surveo aide l'enseignant à conduire une séance en salle informatique
(TP, TD, formation) : savoir quels postes sont présents et où ils se
trouvent dans la salle, suivre l'avancement du travail, distribuer et
ramasser des fichiers, communiquer avec les élèves, reprendre la main
quand il le faut.

> **État du projet : en développement.** Surveo est en cours de refonte
> complète à partir d'un premier prototype. Une première version d'essai
> couvre la séance minimale : plan de salle, miniatures des écrans,
> ouverture, prolongation et clôture d'une séance, retour des postes à
> leur état d'origine.

## Principes

- **Pensé pour la salle de classe.** Surveo montre ce que l'enseignant
  verrait en circulant dans la salle, pendant la séance, et rien de
  plus par défaut.
- **Autonome.** Pas de compte obligatoire, pas de connexion Internet
  obligatoire : un enseignant seul peut s'en servir dans sa salle.
- **Sans droits d'administration.** Installable sur les postes élèves
  même quand l'enseignant n'est pas administrateur ; un déploiement par
  le service informatique reste possible.
- **Transparent.** Les élèves savent quand leur poste est relié à une
  séance, et les postes reviennent à leur état d'origine à la fin.
- **Respectueux des données.** Minimisation, traitements sensibles
  désactivés par défaut et soumis à la décision de l'établissement,
  hébergement en France pour la version hébergée.

## Plateformes visées

- Première version : Windows et Linux.
- Envisagés ensuite : macOS, ChromeOS.

## Construire

Depuis Linux (Fedora), avec Rust, Node.js, les bibliothèques de
développement de WebKitGTK et de PipeWire, et la chaîne MinGW pour
Windows :

```sh
scripts/build.sh            # Linux et Windows, dans dist/
scripts/build.sh --linux-seul
```

Chaque archive contient l'agent, la console (avec son relais intégré) et
une notice d'essai. Les tests : `cargo test` (après une première
construction des interfaces, `cd ui && npm ci && npm run build`).

## Documentation

- [SPEC.md](SPEC.md) : ce que fait Surveo, vu de l'utilisateur.
- [IMPL.md](IMPL.md) : comment c'est réalisé, et dans quel ordre.
- [maquette/console-surveo.html](maquette/console-surveo.html) : maquette
  cliquable de l'interface (console enseignant et poste élève), avec des
  données fictives. À ouvrir dans un navigateur.

## Licence

Surveo est un logiciel libre distribué sous la licence publique de
l'Union européenne **EUPL 1.2** (voir [LICENSE](LICENSE)). Le texte de la
licence existe dans toutes les langues officielles de l'Union, dont le
français, avec la même valeur juridique :
<https://interoperable-europe.ec.europa.eu/collection/eupl/eupl-text-eupl-12>.

« Surveo » est le nom du projet ; son usage pour désigner une version
dérivée n'est pas couvert par la licence du code.
