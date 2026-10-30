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
|   1   | capture.exe     | signé test |    oui "éditeur inconnu" | retenu ~60-130 s puis exécution refusée (« Accès refusé »), pas de quarantaine (Avast) | CyberCapture | au lancement |
|   1   | classifieur.exe | signé test |    oui "éditeur inconnu" | retenu ~64 s puis exécution refusée, pas de quarantaine (Avast) | CyberCapture | au lancement |
|   1   | sans-admin.exe  | signé test |    oui "éditeur inconnu" | retenu ~64-81 s puis exécution refusée, pas de quarantaine (Avast) ; `filtrer` jamais atteint | CyberCapture | au lancement (`installer`) |

Notes :
- `capture.exe` : aucune réaction, probablement parce que le binaire
  avait déjà été exécuté lors des tests précédents (réputation locale) ;
  antivirus confirmé actif. À revérifier avec un binaire au hash neuf.
- `sans-admin.exe` : la détection n'a pas eu lieu au lancement mais au
  moment où l'agent **modifie le proxy de l'utilisateur**. `IDP.Generic`
  est la détection comportementale générique d'Avast, connue pour ses
  faux positifs sur les binaires non signés qui touchent au réseau.

### Passe « signé test » (30 octobre 2026, 12 h 58 - 13 h 08)

Conditions :
- Certificat auto-signé `CN=Surveo Test` (empreinte
  `19FF9DB334498594D0F35987CF4711ED6F587325`), importé dans
  `LocalMachine\Root` ; signature Authenticode SHA-256, horodatée par
  DigiCert. `Get-AuthenticodeSignature` : `Valid` pour les trois.
- Dossier de travail `C:\Users\Public\surveo-antivirus-signe`, copies des
  binaires de référence (empreintes vérifiées contre `empreintes.sha256`
  avant signature). Marque « téléchargé depuis Internet »
  (`Zone.Identifier`, `ZoneId=3`) posée sur les trois, pour que
  SmartScreen soit consulté comme lors de la passe non signée.
- Lancements en ligne de commande depuis la session `mael`
  (administrateur, **non élevé** : jeton filtré par l'UAC), pas depuis le
  compte standard `olive`.
- Defender en **mode passif** (`AMRunningMode : Not running`, temps réel
  désactivé) : Avast est l'antivirus principal du poste, Defender ne
  réagit donc pas en temps réel. Seul SmartScreen est mesuré côté
  Microsoft. Vaut aussi pour la passe non signée.

Empreintes SHA-256 des binaires signés (la signature change le fichier) :

```
badd455ed2471933829bcda85a38f5c80a290a7576ab84f3d8b120c94e62235f  capture.exe
24bbf51f25a6d40aa09feb06d5cb6a1ba18938869bfd6006f836886adee696eb  classifieur.exe
a787b755a0e1063bdb7d890de755ecec52b8dc788f1d837effc8718f4e150373  sans-admin.exe
```

Observations :
- SmartScreen, au double-clic : l'écran apparaît pour les trois, avec
  « Éditeur inconnu » — le certificat de test, bien qu'approuvé
  localement, n'est pas repris par SmartScreen.
- Avast, au double-clic : rien.
- Avast, en ligne de commande : chaque binaire signé est envoyé à
  **CyberCapture** (analyse au Threat Lab) ; le lancement reste suspendu
  ~1 min puis échoue sur « Accès refusé ». Pas de quarantaine, fichiers
  intacts. Une seconde tentative de `sans-admin.exe installer`, dix
  minutes après la première, est toujours refusée (verdict non rendu).
- Contrôle : le `capture.exe` **non signé** (hash de référence, déjà
  connu d'Avast) se lance normalement au même moment (exit 0, 5 s de
  capture). Le blocage tient donc au **hash neuf**, que la signature de
  test ne compense pas.
- `sans-admin.exe` n'a jamais dépassé `installer` : l'étape `filtrer
  bloquer example.com`, celle d'`IDP.Generic`, n'a pas pu être atteinte.
  On ne sait donc pas encore si `IDP.Generic` persiste sur le binaire
  signé.
- Journal `detections.log` d'Avast illisible (verrouillé par
  l'autoprotection) : noms relevés sur les notifications d'Avast.

## Analyse multi-antivirus en ligne (facultatif)

| Binaire | Moteurs / total | Noms de détection |
|---|---|---|
| capture.exe | | |
| classifieur.exe | | |
| sans-admin.exe | | |

## Synthèse (à rédiger après les mesures)

- Comportements réellement en cause :
- Effet de la signature de test (signature valide, sans réputation) :
  **nul, voire défavorable**, sur un seul poste (Windows 11, Avast +
  Defender passif).
  - SmartScreen : l'écran persiste, « Éditeur inconnu ». Une signature
    valide localement ne suffit pas ; il faut un certificat d'une
    autorité reconnue par SmartScreen, et de la réputation.
  - Defender : non mesuré, en mode passif sur ce poste. À refaire sur un
    poste où Defender est l'antivirus actif.
  - Avast : pire que le non signé. En changeant le hash, la signature
    fait d'un binaire connu un binaire neuf, et Avast retient les trois
    dans CyberCapture dès le lancement (le non signé déjà vu passait).
    Avast ne fait confiance ni au certificat de test ni à l'horodatage :
    il juge sur la réputation du hash.
  - `IDP.Generic` : question restée ouverte. CyberCapture bloque avant
    la pose du proxy. À refaire une fois le verdict CyberCapture rendu
    (ou avec le vrai certificat).
  - Conséquence : chaque nouvelle version au hash neuf repart de zéro
    tant que l'éditeur n'a pas de réputation. Sans vrai certificat
    (réputation liée à l'éditeur), il faudrait soumettre chaque build à
    Avast. Ça plaide pour le vrai certificat et des builds reproductibles
    aux empreintes publiées.
- Effet de la signature réelle (OV / EV) :
- Ajustements retenus :
- Critère atteint (aucun blocage d'un binaire signé) : oui / non
