-- Fin de partie décidée par un administrateur (2026-11-15) : onglet Parties,
-- bouton « Fin de partie » (POST /admin/games/{game}/end) quand une partie est
-- bloquée (joueurs partis…). Les stats du moment sont enregistrées sans gagnant,
-- et le salon est vidé.
--
-- games.ended_by_admin_at  date de cet arrêt (NULL = fin normale ou partie en cours) ;
--                          exposé en `ended_by_admin` (récap, historique, game.ended).

ALTER TABLE games ADD COLUMN ended_by_admin_at TEXT;
