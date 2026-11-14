-- Droit à l'effacement et durée de conservation (2026-11-14, checklist point 7 C + E).
--
-- players.last_seen_at  dernière activité connue : connexion, ou requête portant le jeton
--                       du profil (au plus une écriture par joueur et par jour). Un profil
--                       sans activité depuis 24 mois est anonymisé (profile::INACTIVE_DAYS).
--                       Initialisé à la date de la migration : personne n'est purgé avant
--                       24 mois d'inactivité réellement observée.
-- players.deleted_at    profil anonymisé (par le joueur, l'administrateur ou la purge) :
--                       pseudo remplacé par « Joueur supprimé #000123 » (23 caractères,
--                       impossible à saisir : les pseudos font 20 au plus), mot de passe,
--                       photo, emojis, messages, votes d'idées supprimés, jeton révoqué.
--                       La ligne reste pour les parties où il a joué (rounds / actions la
--                       référencent sans ON DELETE) ; sans partie, elle est supprimée.

ALTER TABLE players ADD COLUMN last_seen_at TEXT;
ALTER TABLE players ADD COLUMN deleted_at TEXT;
UPDATE players SET last_seen_at = strftime('%Y-%m-%d %H:%M:%S', 'now');
