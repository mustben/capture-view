# CaptureView — documentation française

Une fenêtre qui affiche le boîtier d'acquisition et joue son audio. Rien de plus.

## Démarrage

1. Brancher le GC550 en USB 3, installer son pilote et connecter/allumer la source HDMI.
2. Fermer OBS ou RECentral si l'un d'eux utilise le boîtier.
3. Lancer `CaptureView.exe`. Une fenêtre indique l’ouverture en cours. Sa fermeture demande l’annulation ; un appel déjà bloqué dans le pilote doit revenir avant la libération du boîtier.

Redimensionner en tirant les bords. Double-cliquer sur l'image ou appuyer sur **F11** pour le plein écran. **Échap** revient à la fenêtre. La croix ou **Alt+F4** ferme l'application et libère le boîtier.

Le son est envoyé à la sortie audio Windows par défaut ; le volume se règle dans Windows. Aucun panneau d'options, compte, réseau, service ou démarrage automatique. Aucune modification du pilote ou d'OBS.

## État de validation

Testé sur Windows 11 x64 avec un **AVerMedia Live Gamer EXTREME GC550** : connexion vidéo 1920×1080, connexion de la broche audio, affichage de l'image fournie par le boîtier, redimensionnement et plein écran. Les tests automatiques initiaux ont utilisé l'image **No Signal** du boîtier ; l'utilisateur a ensuite confirmé le bon fonctionnement en usage réel. La synchronisation prolongée et la latence n'ont pas été mesurées. Les autres modèles ne sont pas annoncés comme compatibles sans test.

Le pilote expose le flux via DirectShow. L'application relie ce flux aux moteurs vidéo/audio de Windows sans faire transiter les images dans des buffers Rust pendant la lecture. Cela évite une couche de traitement inutile, mais ne supprime pas les buffers du pilote ni la latence matérielle.

## Compilation

Installer Rust 1.93.0 (version fixée dans `rust-toolchain.toml`, minimum annoncé : 1.93) et Visual Studio Build Tools (développement C++ de bureau + Windows SDK), puis, depuis le dossier du projet :

```powershell
cargo build --release --locked
.\target\release\capture-view.exe
```

L'exécutable peut être déplacé seul. Le pilote du boîtier reste nécessaire. La compilation release n'ouvre pas de console et ne demande pas les droits administrateur.

## Diagnostic

Journal : `%LOCALAPPDATA%\CaptureView\last-run.log` (remplacé à chaque lancement). Il peut contenir le nom du périphérique et des erreurs techniques : le relire avant de le publier.

- **No Signal** : vérifier source HDMI, câblage et protection HDCP ; le message vient du boîtier.
- **Échec d'ouverture** : fermer les autres applications de capture.
- **Audio indisponible dans le titre** : lire le journal et vérifier la sortie audio Windows.
- Après débranchement : fermer puis relancer l'application.

Un seul boîtier AVerMedia à la fois. Pas d'enregistrement, d'auto-reconnexion, de sélection d'entrée ni de contournement HDCP. Le projet ne distribue aucun pilote ou code propriétaire. Licence MIT. Dépôt : https://github.com/mustben/capture-view ; versions Windows dans la section Releases.

Le packaging (`python scripts/package.py`) recompile la release avec le verrou Cargo, vérifie le binaire PE Windows x64 graphique et prépare les archives et leurs sommes SHA-256 avant de remplacer les anciennes. Les corrections de la revue et leurs limites sont détaillées dans [REVIEW_CORRECTIONS.md](REVIEW_CORRECTIONS.md).
