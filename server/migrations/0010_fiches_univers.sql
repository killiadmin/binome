-- Fiches de présentation des univers (2026-11-14) : une courte fiche par cosmos
-- (Disney, Films saga…), montrée aux joueurs dans le salon quand l'hôte choisit
-- un cosmos et dans les règles (« Univers jouables »). Rédigée par l'admin.
--
-- cosmos.description       courte présentation (NULL = pas de fiche).
-- cosmos.image             image de la fiche, data URI (comme characters.image),
--                          servie par GET /api/cosmos-images/{id}?v=…
-- cosmos.sheet_updated_at  dernière modification de la fiche : version de l'URL de
--                          l'image (activer / désactiver le cosmos ne la change pas).

ALTER TABLE cosmos ADD COLUMN description TEXT;
ALTER TABLE cosmos ADD COLUMN image TEXT;
ALTER TABLE cosmos ADD COLUMN sheet_updated_at TEXT;

UPDATE cosmos SET
    description = 'Les grands classiques Disney, ceux que tout le monde connaît : aucun film de niche ni personnage introuvable. Le cosmos compte aussi quelques personnages Pixar parmi les plus célèbres. Inutile de chercher loin : si tu as grandi devant ces dessins animés, tu les connais.',
    sheet_updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now')
WHERE slug = 'disney';

UPDATE cosmos SET
    description = 'Les sagas de cinéma connues dans le monde entier, celles qui ont marqué le grand écran sur plusieurs films. Rien de confidentiel : uniquement des sagas cultes dont tout le monde a au moins entendu parler.',
    sheet_updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now')
WHERE slug = 'films-saga';
