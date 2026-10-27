<p align="center"><a href="../../README.md">English</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <strong>Français</strong></p>

<p align="center"><img src="../media/hero.png" alt="Petal" width="100%"></p>

# Petal

**Clic droit sur un fichier. Choisis un format. C’est converti.**

Petal est un utilitaire natif pour Windows 11 qui place la conversion d’images, de fichiers audio, de vidéos et de documents texte dans un menu circulaire compact. Il fonctionne dans l’Explorateur et sur le bureau, reprend le thème et la couleur d’accentuation de Windows et permet d’ouvrir le menu contextuel Windows depuis le centre.

Cette page est en français. **L’interface de l’application est en anglais.**

## Aperçu

![Menu circulaire, survol, chargement et validation de Petal](../media/demo.gif)

La démonstration utilise le moteur de rendu natif et un nom de fichier fictif. [Création des visuels](../MEDIA.md) (en anglais).

## Fonctionnalités

- Six formats adaptés au fichier ; son format actuel est remplacé par une autre proposition.
- Conversion locale des images, de l’audio, des vidéos et des documents texte, sans envoi de fichiers.
- Thèmes clair et sombre, couleur d’accentuation Windows, pétales animés et prise en compte de la réduction des animations.
- Chargement visible, coche de validation, notification Windows avec le logo et son désactivable.
- Application Rust native avec Win32/GDI+, sans moteur de navigateur.

> **L’original reste intact.** Le fichier converti est vérifié puis enregistré dans le même dossier avec la nouvelle extension : `photo.jpg` → `photo.webp`. Petal n’écrase jamais un fichier existant. Les métadonnées et la mise en page de la copie convertie peuvent changer.

## Formats

| Fichier sélectionné | Choix par défaut | Alternative au format actuel |
| --- | --- | --- |
| Image | JPG · PNG · WebP · GIF · BMP · PDF | TIFF |
| Audio | MP3 · WAV · FLAC · M4A · OGG · MP4 | OPUS |
| Vidéo | MP4 · MKV · WebM · MOV · MP3 · WAV | AVI |
| Document texte | TXT · DOCX · MD · HTML · RTF · PDF | ODT |

Audio → MP4 crée une vidéo avec un fond noir et le son du fichier. Vidéo → MP3 extrait la piste audio. Les conversions de documents conservent le texte et les paragraphes, plutôt que la mise en page. Les PDF numérisés nécessitent un OCR et sont refusés. Voir les [formats et limites](../../FORMATS.md) (en anglais).

## Installation

**Configuration requise : Windows 11 x64.**

### Télécharger la version v1.0

Télécharge **Petal-v1.0-Windows-x64.zip** depuis la [release v1.0](https://github.com/Syawi/Petal/releases/tag/v1.0). Extrais tout le dossier, puis lance `Petal.exe`. L’archive contient le logiciel, les sources et la documentation. Les outils audio/vidéo s’installent séparément avec `Install-MediaEngine.ps1`.

### Télécharger une compilation

Ouvre une exécution réussie du [workflow Windows](https://github.com/Syawi/Petal/actions/workflows/windows.yml), puis télécharge **Petal-Windows-x64** dans ses artifacts. Une connexion GitHub est requise. Extrais tout le dossier, puis lance `Petal.exe`.

Cette archive prend en charge les images et les documents. Pour activer l’audio et la vidéo, exécute une fois le script inclus dans PowerShell, depuis ce dossier :

```powershell
./Install-MediaEngine.ps1
```

Conserve `assets/` et, après installation, `ffmpeg/` à côté de `Petal.exe`. Aucun accès administrateur ni installation d’Office n’est nécessaire. Petal enregistre un raccourci et une identité de notification pour l’utilisateur courant ; il ne s’ajoute pas au démarrage.

### Compiler les sources

Installe [Rust](https://rustup.rs/) ainsi que les outils de compilation Visual Studio C++ et le SDK Windows pour MSVC, puis :

```powershell
git clone https://github.com/Syawi/Petal.git
cd Petal
./Build.ps1
./Petal.exe
```

Le script prépare FFmpeg/FFprobe, vérifie le SHA-256 du téléchargement, exécute les tests et crée l’exécutable. Pour GNU, consulte [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Utilisation

1. Lance `Petal.exe`. Son icône bleue apparaît dans la zone de notification, parfois parmi les icônes masquées.
2. Fais un clic droit sur un fichier compatible dans l’Explorateur ou sur le bureau.
3. Clique sur un format. Petal crée et vérifie la copie, puis affiche **Conversion complete**.

Le centre ouvre **Windows menu**. **Maj + clic droit** ouvre directement le menu Windows. Clique en dehors de la roue ou appuie sur **Échap** pour la masquer ; une conversion en cours continue en arrière-plan.

| Raccourci | Action |
| --- | --- |
| `1`–`6` | Choisir le pétale correspondant |
| `Tab`, puis `Entrée` / `Espace` | Sélectionner et activer une option |
| `M` | Ouvrir le menu Windows |
| `Échap` | Masquer la roue ou l’animation |

Le menu de l’icône de notification propose **Mute sound** pour couper le son et **Quit Petal** pour quitter. Le choix du son est mémorisé. Quitter est désactivé pendant une conversion.

## Apparence et développement

![Petal avec les thèmes clair et sombre de Windows](../media/themes.png)

Au survol, le pétale ou le centre prend la couleur d’accentuation et son texte devient noir. La réduction des animations désactive les déplacements et utilise un indicateur statique.

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

La CI Windows vérifie le formatage, exécute les tests et compile l’application. Les vérifications du bureau interactif et l’export des images du README sont facultatifs. Documentation technique en anglais : [tests](../../TESTING.md), [contribution](../../CONTRIBUTING.md), [visuels](../MEDIA.md).

## Licence et crédits

[Licence MIT](../../LICENSE), copyright © 2026 Syawi. Les dépendances et ressources conservent leurs licences : [mentions tierces](../../THIRD_PARTY_NOTICES.md), [ressources](../../assets/SOURCES.md), [moteur multimédia](../../ffmpeg/SOURCES.md).
