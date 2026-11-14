# Génération de personnages en masse

Procédure pour créer de nouveaux lots de personnages (proposés, à valider ensuite à la main).

Les lots JSON et leurs images vivent dans `data/characters/` (hors du dépôt : données réelles),
monté en lecture seule dans le conteneur sous `/import` (`docker-compose.yml`). Les commandes
ci-dessous s'exécutent pendant que le serveur tourne. Règles du jeu : `docs/regles-du-jeu.md`.

---

## Prompt à réutiliser

Ces prompts sont aussi disponibles, paramétrables et copiables en un clic, sur la page admin
`/admin/prompts` (lien « Prompts » de la Navbar une fois l'accès personnages débloqué) —
`frontend/src/pages/Admin/AdminPromptsPage.vue`. Garder les deux synchronisés.

Copier-coller ce bloc, ajuster le nombre entre `[ ]` et ajouter éventuellement une
consigne de thème (« plutôt des jeux vidéo », « évite Disney »…).

```
Génère un nouveau lot de [20] personnages (soit [10] binômes) pour le jeu Binome, puis importe-les.

CONTEXTE
- Commande d'import : docker exec binome /binome import-characters /import/<fichier>.json
- Les personnages importés arrivent en verif_manual=false → je les valide ensuite un par un
  dans la popup de /admin/binomes (relecture + correction des mots interdits + import des images).
- L'import est idempotent (clé = univers + nom) et ignore automatiquement les personnages déjà refusés.

RÈGLES DE GÉNÉRATION
- Personnages TRÈS connus uniquement, cohérents avec leur univers et leur cosmos.
- Un binôme = 2 personnages du MÊME univers et du MÊME level_affectation (max 2 par niveau/univers).
- cosmos = sur-catégorie qui regroupe plusieurs univers
  (ex : Disney, Pixar, Marvel, DC Comics, Star Wars, Animé Japonais, Fantasy, Jeux vidéo, Bande dessinée, Série animée).
- 3 mots interdits par personnage, en français, fortement liés au personnage,
  si possible différents entre les 2 membres d'un binôme. Pas de champ "image".
- Format : tableau JSON d'objets { name, universe, cosmos, forbidden_words:[3 chaînes], level_affectation:entier }.

AVANT DE GÉNÉRER
- Liste d'abord TOUS les personnages déjà en base, y compris les masqués (hidden), pour ne rien reproposer :
  docker exec binome /binome list-characters

ENSUITE
- Écris le lot dans data/characters/proposals-AAAA-MM-batch-NN.json (incrémente NN par rapport au dernier).
- Lance l'import et vérifie le rapport (0 erreur attendu).
- Dis-moi combien de binômes sont désormais en attente de validation.
```

---

## Format du fichier JSON

```json
[
  {
    "name": "Pikachu",
    "universe": "Pokémon",
    "cosmos": "Animé Japonais",
    "forbidden_words": ["souris", "éclair", "jaune"],
    "level_affectation": 1
  },
  {
    "name": "Sacha",
    "universe": "Pokémon",
    "cosmos": "Animé Japonais",
    "forbidden_words": ["dresseur", "Pokéball", "casquette"],
    "level_affectation": 1
  }
]
```

- Champs : `name`, `universe`, `cosmos` (facultatif), `forbidden_words` (exactement 3), `level_affectation` (entier ≥ 1).
- `image` facultatif : nom de fichier dans `images/`, chemin absolu ou data URI (pas d'URL).
  En pratique on laisse vide et on importe les images depuis la popup de validation.
- Univers et cosmos manquants sont créés automatiquement. Un cosmos créé ainsi **naît désactivé** :
  l'activer dans l'admin (onglet Parties → Réglages du jeu) une fois ses binômes prêts.

## Convention de nommage des lots

`proposals-AAAA-MM-batch-NN.json` — ex. `proposals-2026-09-batch-02.json`.
Le fichier est conservé comme trace ; l'import restant idempotent, le relancer ne crée pas de doublon.

## Commandes

```bash
# Lister l'existant avant de générer (refusés et « à valider » compris)
docker exec binome /binome list-characters

# Importer un lot
docker exec binome /binome import-characters /import/proposals-AAAA-MM-batch-NN.json

# Compter les binômes en attente de validation
docker exec binome /binome count-pending
```

## Après l'import

1. Ouvrir `/admin/binomes` → la popup « Binômes à valider » s'ouvre s'il y a des propositions.
2. Pour chaque binôme : corriger les mots interdits si besoin, puis **Accepter** (import d'image
   obligatoire pour les 2 personnages, recadrage carré) ou **Refuser** (masqué, jamais reproposé).
3. Un personnage validé devient jouable ; un personnage seul (sans binôme complet) ne l'est pas.

## Rappels règles du jeu

- Un binôme se forme entre 2 personnages du **même univers** partageant le **même `level_affectation`** (2 max par niveau/univers).
- Les **mots interdits** filtrent les questions posées par le joueur qui *possède* ce personnage
  (pas celles qu'on lui pose) : ce sont des mots trop révélateurs de son identité.
- Le tirage ne retient que des personnages `verif_manual = true` et `hidden = false`.
