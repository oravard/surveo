# Prototype `antivirus` — résultats

À remplir à partir de `fiche-test-windows.txt`. Ne rien conclure avant
d'avoir au moins Microsoft Defender + SmartScreen et un antivirus
d'établissement, en non signé puis signé.

## Postes testés

| Poste | OS        | Antivirus + version     | Compte   | Réseau |
|---|---|---|---|---|
| 1     | Windows 11| Defender   4.18.25080.5 + AVAST 26.10.11201.1002 | standard | salle  |

## Réactions observées

Une ligne par (poste × binaire × état de signature). État de signature :
`non signé`, `signé test`, `signé réel`.

| Poste | Binaire         | Signature | SmartScreen          | Antivirus : réaction | Nom de détection | Moment |
|---|---|---|---|---|---|---|
|   1   | capture.exe     | non signé |     oui "non reconnu"|   rien               | — | — (déjà lancé lors des tests précédents) |
|   1   | classifieur.exe | non signé |     oui "non reconnu"|   envoi à l'analyse (cloud) | — | au lancement |
|   1   | sans-admin.exe  | non signé |     oui "non reconnu"|   menace détectée → quarantaine (Avast) | IDP.Generic | à l'étape `filtrer bloquer` (pose du proxy) |

Notes :
- `capture.exe` : aucune réaction, probablement parce que le binaire
  avait déjà été exécuté lors des tests précédents (réputation locale) ;
  antivirus confirmé actif. À revérifier avec un binaire au hash neuf.
- `sans-admin.exe` : la détection n'a pas eu lieu au lancement mais au
  moment où l'agent **modifie le proxy de l'utilisateur**. `IDP.Generic`
  est la détection comportementale générique d'Avast, connue pour ses
  faux positifs sur les binaires non signés qui touchent au réseau.

## Analyse multi-antivirus en ligne (facultatif)

| Binaire | Moteurs / total | Noms de détection |
|---|---|---|
| capture.exe | | |
| classifieur.exe | | |
| sans-admin.exe | | |

## Synthèse (à rédiger après les mesures)

- Comportements réellement en cause :
- Effet de la signature de test (signature valide, sans réputation) :
- Effet de la signature réelle (OV / EV) :
- Ajustements retenus :
- Critère atteint (aucun blocage d'un binaire signé) : oui / non
