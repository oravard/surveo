# Prototype `antivirus` — résultats

À remplir à partir de `fiche-test-windows.txt`. Ne rien conclure avant
d'avoir au moins Microsoft Defender + SmartScreen et un antivirus
d'établissement, en non signé puis signé.

## Postes testés

| Poste | OS        | Antivirus + version     | Compte   | Réseau |
|---|---|---|---|---|
| 1     | Windows 11| Defender   4.18.25080.5 + AVAST 26.10.11201.1002 | standard | salle  |
| 2     | VM Windows 11| Defender 4.18.25100.9008  | admin | VM  |

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
|   2   | capture.exe     | non signé |     oui "non reconnu"|   rien ; capture 30 s menée (1/s, 1,77 % CPU) | — | SmartScreen au 1er lancement ; aucune alerte Defender |
|   2   | classifieur.exe | non signé |     oui "non reconnu"|   rien au lancement (binaire non exécuté : `onnxruntime.dll` absent du kit) | — | à revérifier avec le bundle complet |
|   2   | sans-admin.exe  | non signé |     oui "non reconnu"|   rien (`installer`, `filtrer` proxy fixe, `filtrer --pac`, `desinstaller`) | — | aucune réaction à la pose du proxy, fixe comme PAC |


Notes :
- `capture.exe` : aucune réaction, probablement parce que le binaire
  avait déjà été exécuté lors des tests précédents (réputation locale) ;
  antivirus confirmé actif. À revérifier avec un binaire au hash neuf.
- `sans-admin.exe` : la détection n'a pas eu lieu au lancement mais au
  moment où l'agent **modifie le proxy de l'utilisateur**. `IDP.Generic`
  est la détection comportementale générique d'Avast, connue pour ses
  faux positifs sur les binaires non signés qui touchent au réseau.
- Poste 2 (VM, Defender actif, 30 octobre 2026) : `Get-MpThreatDetection`
  et `Get-MpThreat` **vides** → zéro détection Defender. Defender ne
  réagit **ni à la capture, ni à la pose du proxy (fixe ou PAC), ni à
  l'inscription au démarrage** — contrairement à Avast. Seul SmartScreen
  avertit, au premier lancement. Compte administrateur mais processus
  **non élevé** (`Processus avec droits d'administration : non`),
  opérations au niveau utilisateur. `classifieur.exe` n'a pas pu
  s'exécuter (`onnxruntime.dll` non inclus dans le kit de transfert) : le
  `.exe` a été chargé sans blocage, mais sa lecture de texte n'a pas
  tourné — à revérifier avec le bundle complet.

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

## Synthèse

- **Comportements réellement en cause.** La pose du proxy par un binaire
  inconnu déclenche Avast (`IDP.Generic`) mais **pas Defender**. La
  capture d'écran, la lecture de texte et l'inscription au démarrage
  n'ont déclenché aucun blocage comportemental. Le point commun aux deux
  antivirus est l'écran **SmartScreen « éditeur inconnu »** au premier
  lancement — un problème de signature et de réputation, pas de
  comportement.

- **Microsoft Defender actif (VM Windows 11)** : **aucun blocage**, sur
  binaires non signés. Capture, proxy fixe, proxy PAC, démarrage :
  Defender ne réagit pas (`Get-MpThreat` vide). Reste seulement
  SmartScreen au premier lancement. C'est l'antivirus le plus répandu
  dans les établissements sans produit tiers : pour lui, **la signature
  et la réputation suffisent**, il n'y a pas de comportement à corriger.

- **Effet de la signature de test** (signature valide, sans réputation) :
  **nul, voire défavorable** (poste 1, Avast).
  - SmartScreen : l'écran persiste, « Éditeur inconnu ». Une signature
    valide localement ne suffit pas ; il faut un certificat reconnu par
    SmartScreen et de la réputation.
  - Avast : pire que le non signé. En changeant le hash, la signature
    fait d'un binaire connu un binaire neuf, et Avast retient les trois
    dans CyberCapture dès le lancement. Avast juge sur la réputation du
    hash, pas sur la signature ni l'horodatage.
  - Conséquence : chaque version au hash neuf repart de zéro tant que
    l'éditeur n'a pas de réputation. D'où **vrai certificat** (réputation
    liée à l'éditeur), **builds reproductibles** et **empreintes
    publiées**, et **soumission des faux positifs** aux éditeurs.

- **Effet de la signature réelle (certificat d'une autorité reconnue)** :
  **reste à mesurer** — c'est la dernière inconnue. On attend qu'elle
  lève, avec le temps et le volume, l'avertissement SmartScreen, et
  qu'elle évite le blocage CyberCapture/`IDP.Generic` d'Avast (à
  confirmer, après soumission du faux positif). L'EV n'apporte plus de
  réputation SmartScreen immédiate depuis mars 2024 : OV suffit.

- **Ajustements retenus :**
  - Signer en Authenticode tous les exécutables distribués, éditeur
    constant, horodatage ; laisser la réputation monter (temps + volume).
  - Builds reproductibles, empreintes publiées ; soumettre les faux
    positifs aux éditeurs tiers (Avast en premier).
  - Mode déployé : exclusion posée par l'établissement — voie robuste,
    indépendante de la réputation, utile dès le lancement.
  - Documentation pour le service informatique (comportement, chemins,
    empreintes).

- **Critère atteint (aucun blocage d'un binaire signé)** :
  - Defender (le plus répandu) : **pas de blocage comportemental** ;
    reste la réputation SmartScreen, qui se gagne dans le temps. Critère
    tenu dès que l'éditeur a de la réputation.
  - Avast (antivirus tiers) : **non encore démontré** ; la signature de
    test ne suffit pas, le vrai certificat reste à essayer.
