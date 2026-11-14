-- Boîte à idées (2026-11-14), onglet Idées.
--
-- ideas            idée postée par un joueur (texte libre, filtré des injures, 5 par 24 h).
--   player_id      auteur ; NULL si le joueur a été supprimé depuis (l'idée reste,
--                  signée author_name).
--   author_name    pseudo de l'auteur au moment du post (affiché si le joueur n'existe plus ;
--                  sinon son pseudo actuel est relu).
--   pinned_at      mise en favori par un administrateur : en tête de liste ; NULL = non.
-- idea_votes       un +1 par joueur et par idée (clé primaire), retirable ; jamais sur sa
--                  propre idée (règle du service).

CREATE TABLE ideas (
    id          INTEGER PRIMARY KEY,
    player_id   INTEGER REFERENCES players (id) ON DELETE SET NULL,
    author_name TEXT NOT NULL,
    body        TEXT NOT NULL,
    pinned_at   TEXT,
    created_at  TEXT,
    updated_at  TEXT
);
CREATE INDEX ideas_player_id_created_at ON ideas (player_id, created_at);

CREATE TABLE idea_votes (
    idea_id    INTEGER NOT NULL REFERENCES ideas (id) ON DELETE CASCADE,
    player_id  INTEGER NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    created_at TEXT,
    PRIMARY KEY (idea_id, player_id)
);
CREATE INDEX idea_votes_player_id ON idea_votes (player_id);
