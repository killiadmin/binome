-- Administrateurs désignés (2026-11-14), onglet Joueurs de l'admin.
--
-- players.admin_granted_at  le propriétaire (profil BINOME_ADMIN_PSEUDO, mot de passe
--                     d'administration) a désigné ce joueur administrateur ; NULL = non.
--                     L'administrateur désigné accède à l'administration avec son jeton
--                     de profil, à condition que son profil ait un mot de passe ; il ne
--                     peut pas désigner d'autres administrateurs.

ALTER TABLE players ADD COLUMN admin_granted_at TEXT;
