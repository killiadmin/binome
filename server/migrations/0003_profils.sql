-- Profils joueurs : connexion par pseudo, mot de passe facultatif.
--
-- password_hash      empreinte PBKDF2 (support/password.rs) ; NULL = profil public
--                    (n'importe qui peut s'y connecter avec le pseudo seul).
-- password_prompted  1 dès qu'on a proposé au joueur de protéger son profil
--                    (proposition faite une seule fois, acceptée ou non).
-- token_version      entre dans la signature des jetons de profil : l'incrémenter
--                    (pose ou changement de mot de passe) invalide les jetons émis.
--
-- app_secrets : secrets générés par le serveur au premier démarrage (clé de
-- signature des jetons de profil). Ils voyagent avec les sauvegardes de la base.

ALTER TABLE players ADD COLUMN password_hash TEXT;
ALTER TABLE players ADD COLUMN password_prompted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE players ADD COLUMN token_version INTEGER NOT NULL DEFAULT 0;

CREATE TABLE app_secrets (
    name   TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);
