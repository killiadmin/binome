-- Contrôles de l'administrateur (2026-11-14), onglets Parties et Joueurs de l'admin.
--
-- app_settings        réglages globaux, valeur JSON. `game_modes` : modes de jeu
--                     proposés aux hôtes (« random » = Aléatoire, « cosmos » = Cosmos imposé).
-- cosmos.is_enabled   cosmos proposé aux hôtes et tiré en mode Aléatoire. Un cosmos
--                     créé ensuite (admin ou import) naît désactivé : l'administrateur
--                     l'active une fois ses données prêtes.
-- disabled_binomes    binômes de personnages (univers + niveau d'affectation)
--                     écartés du tirage par l'administrateur.
-- players.validated_at  validation manuelle du profil par l'administrateur ; NULL =
--                     non validé : pas d'historique ni d'emojis personnalisés (images).
--                     Le profil administrateur est toujours considéré validé.
--
-- État initial voulu par l'administrateur : seul « Cosmos imposé », seuls Disney et
-- Films saga, aucun profil validé.

CREATE TABLE app_settings (
    name   TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);
INSERT INTO app_settings (name, value) VALUES ('game_modes', '["cosmos"]');

ALTER TABLE cosmos ADD COLUMN is_enabled INTEGER NOT NULL DEFAULT 0;
UPDATE cosmos SET is_enabled = 1 WHERE slug IN ('disney', 'films-saga');

CREATE TABLE disabled_binomes (
    universe_id        INTEGER NOT NULL REFERENCES universes (id) ON DELETE CASCADE,
    level_affectation  INTEGER NOT NULL,
    created_at         TEXT,
    PRIMARY KEY (universe_id, level_affectation)
);

ALTER TABLE players ADD COLUMN validated_at TEXT;
