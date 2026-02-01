# Instructions pour les agents

## Qualité et robustesse
- Le code doit être robuste, maintenable et adapté à un usage professionnel.
- Appliquer une approche d'ingénierie logicielle rigoureuse (validation d'entrées, gestion d'erreurs, cas limites).

## Style et documentation
- Le code ajouté ou modifié doit être **commenté en français**.
- Tout nouveau commentaire doit être rédigé en français.

## Tests
- Créer des **tests unitaires** pour toute fonctionnalité ajoutée ou modifiée.
- **Exécuter les tests** à chaque modification et rapporter les résultats.

## Contexte produit
- Nous développons un logiciel pour transporter Codex dans un terminal sur clé USB,
  afin de l'utiliser sur n'importe quel PC **sans installation** sur la machine hôte.

## Technologies et composants clés du projet
- **Python 3.10+** comme langage principal (voir `pyproject.toml`).  
- **Textual** (`textual[syntax]`) pour l'interface TUI, avec styles **`.tcss`** pour le rendu.  
- **Packaging** via `setuptools` et point d'entrée CLI `usbide` (script console).  
- Organisation du code dans le paquet `usbide/` avec modules applicatifs dédiés.  

## Exigences de performance cognitive
- Toujours **analyser le contexte** avant d'agir.  
- Vérifier les hypothèses, couvrir les cas limites et expliciter les choix techniques.  
