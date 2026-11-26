-- Emojis privés / publics (2026-11-25). Un emoji image créé par un joueur naît
-- privé : il est dans sa barre seulement et lui seul peut l'envoyer (tout le monde
-- le voit monter en partie). Un profil validé par l'administrateur peut le rendre
-- public : il rejoint alors la barre de tous les joueurs.
--
-- custom_reactions.is_public  1 = dans la barre de tous ; 0 = réservé au créateur
--                             (created_by_pseudo). Les emojis Unicode et ceux de
--                             l'admin sont publics.
--
-- Les emojis existants étaient partagés avec tous : ils restent publics.

ALTER TABLE custom_reactions ADD COLUMN is_public INTEGER NOT NULL DEFAULT 0;
UPDATE custom_reactions SET is_public = 1;
