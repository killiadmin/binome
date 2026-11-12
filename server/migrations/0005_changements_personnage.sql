-- Demandes de changement de personnage par joueur, plafonnées (MAX_CHARACTER_CHANGES).
--
-- Les binômes sont supprimés et recréés à chaque retirage : le compteur ne peut pas
-- vivre dans `binome_player`. Il est gardé dans la partie, en JSON
-- `{"<player_id>": <demandes acceptées>}` (comme `rejected_character_ids`).
-- NULL pour les parties existantes = aucune demande comptée.

ALTER TABLE games ADD COLUMN character_changes TEXT;
