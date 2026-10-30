# Signer les binaires pour le test

Deux signatures, deux usages :

- **Signature de test** (certificat auto-signé) : seulement pour mesurer
  l'effet d'une *signature valide* sur l'antivirus, avant d'avoir le vrai
  certificat. Le certificat de test doit être importé comme autorité de
  confiance **dans la VM de test uniquement** ; il n'apporte aucune
  réputation et ne doit jamais servir à distribuer.
- **Signature réelle** : le certificat de signature de code du projet,
  acheté auprès d'une autorité (VISION.md §5). C'est lui qui compte pour
  le critère de réussite et pour la réputation SmartScreen.

## Signature de test

### Depuis Windows (dans la VM de test)

```powershell
# 1. Créer un certificat de test et l'ajouter aux autorités de confiance de la VM
$cert = New-SelfSignedCertificate -Type CodeSigningCert `
  -Subject "CN=Surveo Test" -CertStoreLocation Cert:\CurrentUser\My
Export-Certificate -Cert $cert -FilePath surveo-test.cer
Import-Certificate -FilePath surveo-test.cer -CertStoreLocation Cert:\LocalMachine\Root

# 2. Signer, avec horodatage
$ts = "http://timestamp.digicert.com"
Set-AuthenticodeSignature -FilePath .\capture.exe      -Certificate $cert -TimestampServer $ts
Set-AuthenticodeSignature -FilePath .\classifieur.exe  -Certificate $cert -TimestampServer $ts
Set-AuthenticodeSignature -FilePath .\sans-admin.exe   -Certificate $cert -TimestampServer $ts

# 3. Vérifier
Get-AuthenticodeSignature .\capture.exe, .\classifieur.exe, .\sans-admin.exe | Format-List
```

### Depuis le poste de développement (Fedora)

`osslsigncode` signe un PE depuis Linux (paquet `osslsigncode`, absent
pour l'instant — `sudo dnf install osslsigncode`).

```sh
# certificat de test
openssl req -x509 -newkey rsa:3072 -keyout test.key -out test.crt -days 365 -nodes \
  -subj "/CN=Surveo Test"
# signer (répéter pour chaque binaire), avec horodatage
osslsigncode sign -certs test.crt -key test.key -h sha256 \
  -t http://timestamp.digicert.com \
  -in capture.exe -out capture-signe.exe
```

Importer ensuite `test.crt` dans les autorités de confiance de la VM
Windows (sinon la signature est « non approuvée » et le test ne mesure
pas ce qu'on veut).

## Signature réelle

Mêmes commandes avec le certificat du projet (fichier `.pfx` ou jeton
matériel selon l'autorité). Noter dans `resultats.md` le type de
certificat (OV ou EV), car la réputation SmartScreen en dépend :

- **EV** : réputation immédiate, pas d'écran SmartScreen « éditeur
  inconnu ».
- **OV** : réputation qui monte avec le nombre d'installations ; prévoir
  la soumission des binaires au portail de l'éditeur pour accélérer.
