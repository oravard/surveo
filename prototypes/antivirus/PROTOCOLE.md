# Prototype `antivirus` — protocole

Jetable. Ce prototype ne produit pas de code : il **mesure comment les
antivirus réagissent à l'agent** et établit la **voie de reconnaissance**
d'un logiciel légitime. Il répond à la question du tableau de phase 2
(IMPL.md) : *comment Microsoft Defender et les antivirus courants des
établissements réagissent-ils à l'agent, signé ou non ?*

Critère de réussite : **aucun blocage d'un binaire signé** ; sinon, liste
des comportements en cause et des ajustements.

## Position de principe

Surveo est un logiciel libre et transparent : l'élève sait quand son
poste est relié, le service informatique peut lire le code et vérifier ce
que fait l'agent. L'objectif de ce prototype est donc de **faire
reconnaître l'agent**, pas de le dissimuler. Les techniques de
contournement d'antivirus (empaquetage/packing, obscurcissement du code,
anti-analyse) sont **écartées** : elles contredisent la transparence du
projet, dégradent la réputation auprès des éditeurs d'antivirus et
seraient de toute façon vaines sur un poste de salle géré. La seule voie
retenue est la voie honnête : signature, réputation, documentation,
exclusions décidées par l'établissement.

## Comportements de l'agent qui peuvent alerter un antivirus

Tous légitimes et déjà prototypés ; chacun ressemble, isolément, à une
famille de logiciels indésirables :

| Comportement | Prototype | Ce à quoi ça ressemble |
|---|---|---|
| Binaire inconnu, non signé | tous | éditeur inconnu (SmartScreen) ; premier facteur, déjà constaté |
| Capture d'écran répétée | `capture` | espiogiciel de capture |
| Lecture du texte pour le classifieur (module sensible) | `classifieur` | enregistreur de frappe |
| Modification du proxy de l'utilisateur | `sans-admin` | détournement de proxy (adware) |
| Inscription au démarrage de session (clé `Run`) | `sans-admin` | persistance |
| Connexions sortantes chiffrées vers un relais | `relais` | balise réseau |
| Binaire compilé avec MinGW (`capture.exe`) | `capture` | mauvaise réputation fréquente des binaires MinGW |

## Binaires soumis au test

Les binaires **déjà produits** par les autres prototypes, non signés,
dont les empreintes sont dans `empreintes.sha256` :

- `capture.exe` (prototype 2) — capture d'écran ;
- `classifieur.exe` (prototype 5) — analyse de texte locale ;
- `sans-admin.exe` (prototype 3) — proxy utilisateur + démarrage de session.

On teste chaque binaire dans trois états, pour isoler l'effet de la
signature :

1. **non signé** (état actuel) ;
2. **signé avec un certificat de test** auto-signé, importé comme
   autorité de confiance dans la VM (`signer-test.md`) — mesure l'effet
   « présence d'une signature valide » sans réputation ;
3. **signé avec le vrai certificat de signature de code** du projet
   (OV, puis EV si possible) — mesure l'effet « signature + réputation ».
   Dépend de l'achat du certificat (VISION.md §5).

## Antivirus à couvrir

- **Microsoft Defender** (activé par défaut sous Windows 10/11) et
  **SmartScreen** : le cas le plus important, présent partout.
- Antivirus que l'on rencontre dans les établissements et sur les postes
  gérés par la Région : au moins un produit grand public (Avast — déjà vu
  bloquer nos binaires non signés) et, si l'on y a accès, un produit
  d'entreprise (par ex. ESET, Kaspersky, Bitdefender, Trend Micro).
  Noter la marque et la version exacte de chaque poste réel testé.

## Déroulé

1. Sur chaque poste/VM, relever l'antivirus présent et sa version.
2. Lancer les trois binaires non signés en suivant
   `fiche-test-windows.txt` ; noter tout message de l'antivirus et de
   SmartScreen (blocage, mise en quarantaine, analyse qui traîne, simple
   avertissement « éditeur inconnu »).
3. Recommencer avec les binaires signés de test, puis avec les binaires
   signés réels quand le certificat est disponible.
4. Consigner dans `resultats.md`, une ligne par (poste × antivirus ×
   binaire × état de signature).

Les mesures qui demandent de vrais postes de salle (postes Région sans
droits d'administration) se font sur place ; les autres en VM.

## Voie de reconnaissance visée (ce que le prototype doit confirmer)

- **Signer en Authenticode** tous les exécutables distribués, avec
  horodatage. Un certificat **EV** donne une réputation SmartScreen
  immédiate ; un certificat **OV** demande une montée en réputation sur
  quelques semaines/installations.
- **Stabiliser la réputation** : éditeur constant, empreintes publiées,
  et **soumission des binaires aux éditeurs d'antivirus** comme connus et
  sains (portail de soumission Microsoft ; formulaires de faux positif
  des autres éditeurs).
- **Documenter pour le service informatique** : comportement de l'agent,
  chemins d'installation, empreintes, builds reproductibles (SPEC.md
  7.10) — pour qu'une exclusion soit posée en connaissance de cause.
- **Mode déployé** : l'établissement pose l'exclusion par ses moyens
  (stratégie de groupe, console de l'antivirus) ; voie la plus robuste.
- **Module sensible** (classifieur/lecture de texte) : rappeler que
  l'analyse est locale, que rien n'est exfiltré et que le modèle n'est
  pas livré par défaut (SPEC.md 5.10) — argument d'acceptation.
