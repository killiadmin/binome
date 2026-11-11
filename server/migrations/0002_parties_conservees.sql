-- Les parties survivent à la suppression de leur salon.
--
-- Avant : games.room_id ON DELETE CASCADE — quand le dernier joueur quittait un
-- salon, le salon était supprimé avec toutes ses parties (historique, scores,
-- classement). Après : la référence est vidée (ON DELETE SET NULL) et la partie
-- reste dans l'historique ; seul le code du salon disparaît.
--
-- SQLite ne modifie pas une contrainte existante : la table est reconstruite
-- (procédure officielle, clés étrangères suspendues par le serveur et
-- revérifiées avant validation). Les ids et la séquence AUTOINCREMENT sont gardés.

CREATE TEMP TABLE sequence_games AS SELECT seq FROM sqlite_sequence WHERE name = 'games';

CREATE TABLE games_nouvelle (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id                  INTEGER REFERENCES rooms (id) ON DELETE SET NULL,
    status                   TEXT NOT NULL DEFAULT 'waiting',
    characters_confirmed_at  TEXT,
    draw_number              INTEGER NOT NULL DEFAULT 1,
    rejected_character_ids   TEXT,
    created_at               TEXT,
    updated_at               TEXT
);

INSERT INTO games_nouvelle (id, room_id, status, characters_confirmed_at, draw_number, rejected_character_ids, created_at, updated_at)
SELECT id, room_id, status, characters_confirmed_at, draw_number, rejected_character_ids, created_at, updated_at FROM games;

DROP TABLE games;
ALTER TABLE games_nouvelle RENAME TO games;
CREATE INDEX games_room_id ON games (room_id);
CREATE INDEX games_status ON games (status);

-- Pas de réutilisation des ids de parties supprimées
UPDATE sqlite_sequence SET seq = max(seq, (SELECT seq FROM sequence_games)) WHERE name = 'games';
DROP TABLE sequence_games;
