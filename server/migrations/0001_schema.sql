-- Schéma métier de Binome (SQLite). Repris de MySQL à l'identique — mêmes tables,
-- colonnes, ids, contraintes et cascades — moins le code mort et les tables
-- techniques de Laravel (users, sessions, cache, jobs…).
--
-- Collation BINOME_CI (déclarée par le serveur à l'ouverture de chaque connexion) :
-- insensible à la casse, aux accents et aux espaces finaux, comme
-- `utf8mb4_unicode_ci` sous MySQL. Elle porte sur les colonnes que l'application
-- compare ou trie (pseudos, codes, noms, slugs) : « Killian » = « killián ».
--
-- Dates : texte UTC « YYYY-MM-DD HH:MM:SS » (précision de MySQL TIMESTAMP).
-- Booléens : INTEGER 0 / 1. Listes JSON : TEXT.

CREATE TABLE players (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pseudo      TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    created_at  TEXT,
    updated_at  TEXT
);

CREATE TABLE cosmos (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    slug        TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    created_at  TEXT,
    updated_at  TEXT
);

CREATE TABLE universes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    slug        TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    cosmos_id   INTEGER REFERENCES cosmos (id) ON DELETE SET NULL,
    created_at  TEXT,
    updated_at  TEXT
);
CREATE INDEX universes_cosmos_id ON universes (cosmos_id);

CREATE TABLE characters (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    universe_id        INTEGER NOT NULL REFERENCES universes (id) ON DELETE CASCADE,
    level_affectation  INTEGER NOT NULL DEFAULT 1,
    verif_manual       INTEGER NOT NULL DEFAULT 0,
    hidden             INTEGER NOT NULL DEFAULT 0,
    name               TEXT NOT NULL COLLATE BINOME_CI,
    slug               TEXT NOT NULL COLLATE BINOME_CI,
    image              TEXT,
    forbidden_words    TEXT NOT NULL,
    created_at         TEXT,
    updated_at         TEXT,
    UNIQUE (universe_id, slug)
);

CREATE TABLE rooms (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    code                  TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    is_private            INTEGER NOT NULL DEFAULT 0,
    is_locked             INTEGER NOT NULL DEFAULT 0,
    max_players           INTEGER NOT NULL DEFAULT 8,
    game_mode             TEXT NOT NULL DEFAULT 'random',
    cosmos_id             INTEGER REFERENCES cosmos (id) ON DELETE SET NULL,
    reveal_to_eliminated  INTEGER NOT NULL DEFAULT 1,
    best_question_vote    INTEGER NOT NULL DEFAULT 1,
    created_by            INTEGER NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    created_at            TEXT,
    updated_at            TEXT
);
CREATE INDEX rooms_created_by ON rooms (created_by);

CREATE TABLE room_player (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id     INTEGER NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    player_id   INTEGER NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    is_ready    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT,
    updated_at  TEXT,
    UNIQUE (room_id, player_id)
);
CREATE INDEX room_player_player_id ON room_player (player_id);

CREATE TABLE games (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id                  INTEGER NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    status                   TEXT NOT NULL DEFAULT 'waiting',
    characters_confirmed_at  TEXT,
    draw_number              INTEGER NOT NULL DEFAULT 1,
    rejected_character_ids   TEXT,
    created_at               TEXT,
    updated_at               TEXT
);
CREATE INDEX games_room_id ON games (room_id);
CREATE INDEX games_status ON games (status);

CREATE TABLE binomes (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id      INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    universe_id  INTEGER NOT NULL REFERENCES universes (id),
    is_orphan    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT,
    updated_at   TEXT
);
CREATE INDEX binomes_game_id ON binomes (game_id);

CREATE TABLE binome_player (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    binome_id         INTEGER NOT NULL REFERENCES binomes (id) ON DELETE CASCADE,
    player_id         INTEGER NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    character_id      INTEGER NOT NULL REFERENCES characters (id),
    character_vote    TEXT,
    score             INTEGER NOT NULL DEFAULT 0,
    is_eliminated     INTEGER NOT NULL DEFAULT 0,
    is_excluded       INTEGER NOT NULL DEFAULT 0,
    eliminated_round  INTEGER,
    created_at        TEXT,
    updated_at        TEXT
);
CREATE INDEX binome_player_binome_id ON binome_player (binome_id);
CREATE INDEX binome_player_player_id ON binome_player (player_id);
CREATE INDEX binome_player_character_id ON binome_player (character_id);

CREATE TABLE rounds (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id            INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    number             INTEGER NOT NULL,
    current_player_id  INTEGER NOT NULL REFERENCES players (id),
    is_finished        INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT,
    updated_at         TEXT
);
CREATE INDEX rounds_game_id ON rounds (game_id);
CREATE INDEX rounds_current_player_id ON rounds (current_player_id);

CREATE TABLE actions (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    round_id               INTEGER NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    player_id              INTEGER NOT NULL REFERENCES players (id),
    target_player_id       INTEGER REFERENCES players (id),
    type                   TEXT NOT NULL,
    content                TEXT,
    character_name         TEXT,
    is_valid               INTEGER NOT NULL,
    accusation_correct     INTEGER,
    accusation_confirmed   INTEGER,
    answer                 TEXT,
    answer_corrected_from  TEXT,
    answer_corrected_at    TEXT,
    card_draw_status       TEXT,
    card_count             INTEGER,
    card_draw_deadline     TEXT,
    drawn_word             TEXT,
    created_at             TEXT,
    updated_at             TEXT
);
CREATE INDEX actions_round_id ON actions (round_id);
CREATE INDEX actions_player_id ON actions (player_id);
CREATE INDEX actions_target_player_id ON actions (target_player_id);
CREATE INDEX actions_card_draw_status ON actions (card_draw_status);

CREATE TABLE question_votes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    round_id    INTEGER NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    action_id   INTEGER NOT NULL REFERENCES actions (id) ON DELETE CASCADE,
    voter_id    INTEGER NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    created_at  TEXT,
    updated_at  TEXT,
    UNIQUE (round_id, voter_id)
);
CREATE INDEX question_votes_action_id ON question_votes (action_id);
CREATE INDEX question_votes_voter_id ON question_votes (voter_id);

CREATE TABLE game_stats (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id             INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    player_pseudo       TEXT NOT NULL COLLATE BINOME_CI,
    character_name      TEXT NOT NULL,
    eliminations        INTEGER NOT NULL DEFAULT 0,
    best_questions      INTEGER NOT NULL DEFAULT 0,
    rounds_survived     INTEGER NOT NULL DEFAULT 0,
    survived_full_game  INTEGER NOT NULL DEFAULT 0,
    score               INTEGER NOT NULL DEFAULT 0,
    is_winner           INTEGER NOT NULL DEFAULT 0,
    is_eliminated       INTEGER NOT NULL DEFAULT 0,
    is_excluded         INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT,
    updated_at          TEXT
);
CREATE INDEX game_stats_game_id ON game_stats (game_id);
CREATE INDEX game_stats_player_pseudo ON game_stats (player_pseudo);

CREATE TABLE chat_messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id    INTEGER REFERENCES players (id) ON DELETE SET NULL,
    author_name  TEXT NOT NULL,
    author_key   TEXT NOT NULL,
    body         TEXT NOT NULL,
    created_at   TEXT,
    updated_at   TEXT
);
CREATE INDEX chat_messages_created_at ON chat_messages (created_at);
CREATE INDEX chat_messages_player_id ON chat_messages (player_id);

CREATE TABLE player_avatars (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pseudo      TEXT NOT NULL COLLATE BINOME_CI UNIQUE,
    image       TEXT NOT NULL,
    created_at  TEXT,
    updated_at  TEXT
);

-- emoji en comparaison binaire : en collation insensible, tous les emojis
-- seraient égaux entre eux (même raison que utf8mb4_bin sous MySQL).
CREATE TABLE custom_reactions (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    name               TEXT NOT NULL,
    emoji              TEXT COLLATE BINARY,
    image              TEXT,
    created_by_pseudo  TEXT COLLATE BINOME_CI,
    is_enabled         INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT,
    updated_at         TEXT
);
CREATE INDEX custom_reactions_enabled ON custom_reactions (is_enabled, created_at);
