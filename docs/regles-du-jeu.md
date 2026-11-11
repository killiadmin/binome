# Règles du jeu et comportements

> Repris du README de l'ancien backend Laravel (supprimé en phase 7 de la migration ; dernière
> version : étiquette git `laravel-final`). Le serveur Rust applique ces règles à l'identique,
> vérifiées par le banc `tests/e2e/`.

## Règles

### Objectif
Deviner le personnage secret des autres joueurs avant que son propre binome soit découvert.

### Mise en place
- Nombre de joueurs : **minimum 4**, pair ou impair
- Au début de chaque partie, chaque joueur reçoit un **personnage secret** tiré d'un univers (ex : Disney, Marvel…)
- Les joueurs sont regroupés en **binomes** de 2 personnes partageant le même univers, **à leur insu**
- Chaque personnage possède **3 mots interdits**

#### L'orphelin (nombre impair)

Si les joueurs sont en nombre impair, le joueur surnuméraire du tirage se retrouve
**seul dans son binome** : c'est l'**orphelin** (`binomes.is_orphan = true`, un seul
`binome_player`). Il reçoit un personnage et un univers comme les autres, dans un
univers qui aurait pu porter un vrai binome — rien ne le distingue.

- **L'orphelin ignore qu'il est orphelin** : aucune route ni aucun event ne le lui dit
  avant la fin. `GET /games/{game}` et l'event `GameStarted` exposent seulement
  `has_orphan: true` — un orphelin existe, sans dire qui.
- Il compte comme **une équipe à part entière** : tant qu'il est en vie son équipe est
  intacte, donc s'il est le dernier en jeu il gagne **et touche le bonus de +5**, comme
  un binome complet. C'est ce qui le distingue d'un rescapé (joueur dont le partenaire
  a été éliminé), qui gagne sans bonus.
- Il est révélé à la fin, dans `GameEnded.all_binomes[].is_orphan`.

### Modes de jeu

L'hôte choisit le mode dans le lobby (`PATCH /rooms/{room}/settings`) :

| Mode (`rooms.game_mode`) | Comportement |
|---|---|
| `random` (défaut) | Chaque binome reçoit un **univers tiré au hasard** parmi les univers jouables. |
| `cosmos` | L'hôte fixe un **cosmos** (`rooms.cosmos_id`) : tous les binomes reçoivent un univers **de ce cosmos**. |

Dans les deux modes : **un univers distinct par binome**, et binome = 2 persos du même univers au
même `level_affectation`. Un univers est « jouable » s'il a ≥ 2 personnages `playable()` partageant
un niveau. Le nombre de personnages par niveau n'est **pas** limité en saisie manuelle (fiche /
édition) pour permettre toute correction d'affectation ; si un niveau en compte plus de 2, le
tirage en prend 2 au hasard et la liste `/admin/binomes` signale l'anomalie. L'orphelin consomme lui aussi un univers : le mode `cosmos` exige donc
**≥ ceil(nb de joueurs / 2) univers jouables** dans le cosmos, sinon le démarrage renvoie une
erreur `422` (message nommant le cosmos et les compteurs).
`GET /cosmos` expose `playable_universe_count` par cosmos pour l'affichage côté lobby.

### Vote des personnages
Avant le round 1, chaque joueur **accepte son personnage** ou **demande à en changer** : un joueur
qui ne connaît pas son personnage donnerait de fausses réponses. Les deux votes sont publics
(compteurs « Prêts : x / n » et « Demandes à changer : y / n ») et **rien ne se passe tant que
tous n'ont pas voté** (`binome_player.character_vote` : `null` | `accepted` | `change`). Ensuite :

- **Au moins une demande de changement** → binomes, univers et personnages sont **retirés pour
  tout le monde** (`games.draw_number` + 1) et chacun revote. Les personnages refusés sont notés
  dans `games.rejected_character_ids` et écartés des tirages suivants — sauf si le catalogue ne
  suffit plus sans eux, auquel cas on tire sur tous les personnages.
- **Tous ont accepté** → `games.characters_confirmed_at` est renseigné et le round 1 est créé.
- Un vote est définitif pour le tirage en cours.
- Tant que le vote dure, il n'existe aucun round : personne ne peut jouer, et l'hôte ne peut
  ni passer un tour ni exclure (`409`). Un joueur déconnecté bloque donc le vote jusqu'à son retour.

### Déroulement
- Les joueurs jouent **chacun leur tour**, dans le même ordre à chaque round
- À son tour, un joueur peut effectuer **une seule action** parmi deux :
    1. **Poser une question** à n'importe quel autre joueur (réponse libre : oui/non)
    2. **Faire une accusation** : désigner un joueur et nommer précisément son personnage

#### « Je ne sais pas » → tirage de carte
- Si la cible répond **« Je ne sais pas »**, le questionneur (A) **tire une carte face cachée**
  parmi les mots interdits de la cible (B) : un mot tiré **au hasard** (le paquet est complet à
  chaque tirage, un mot peut retomber), que **lui seul** voit pendant **5 secondes**
- A décide ensuite de **montrer ce mot à tous les autres joueurs, sauf B** (eux aussi le voient
  5 s), ou de le **garder pour lui**
- B sait qu'une carte a été tirée, mais **jamais laquelle**, ni si elle a été montrée
- Le tour **attend** A : il a **30 s** (`CARD_DRAW_SECONDS` (`server/src/services/play.rs`)) pour tirer et décider,
  sinon la carte est perdue et le tour avance. Une cible sans mot interdit → pas de tirage

### Mots interdits
- Chaque joueur a **ses propres mots interdits** liés à **son personnage**, connus de lui seul
- Ce sont les mots interdits de la **cible** d'une question qui comptent ; le questionneur ne les connaît pas
- La question n'est **jamais refusée** côté serveur (`server/src/services/play.rs`) : le questionneur
  ne doit pas apprendre qu'il a fauté. La détection est faite **côté front, chez la cible uniquement**
  (`frontend/src/services/forbiddenWords.js`), dans la modale de réponse :
  - **exact (100 %)** — un mot de la question est le mot interdit, à la casse, aux accents et au
    pluriel (s/x) près → la cible **doit mentir** (répondre l'inverse de la vérité)
  - **proche (≥ 80 %)** — faute de frappe, conjugaison, dérivé (Levenshtein + racinisation légère)
    → le mensonge est **légal**, la cible juge
  - Exemple : Peter Pan (`costume`, `poudre`, `voler`) — « Peut-il voler ? » → exact ;
    « Est-ce que ton personnage vol ? » → proche
- Purement indicatif : aucun bouton n'est désactivé. La cible reste libre de répondre honnêtement
  pour une faute de frappe évidente du questionneur ou un cas exceptionnel.
- `server/src/support/forbidden.rs` est le port Rust du même algorithme (récap de fin de partie,
  trophée « Permis de mentir » : exact + proche) — garder les deux synchronisés.

### Victoire
- Quand une accusation est correcte, le **joueur ciblé est éliminé** (son binome n'est pas révélé)
- On appelle **équipe intacte** un binome dont aucun membre n'a été éliminé : une paire complète,
  ou l'orphelin encore en vie. Un **rescapé** est un joueur encore actif dont le partenaire est tombé.
- La partie se termine quand il ne reste **qu'une seule équipe intacte et aucun rescapé** → cette
  équipe gagne ; **ou** plus aucune équipe intacte et **un seul rescapé** → il gagne seul
- Bonus de score : **+5 pour une victoire avec l'équipe intacte** — donc pour un binome complet
  ou pour l'orphelin — et **+3 pour un rescapé** qui gagne seul (sinon il pouvait finir à égalité
  avec des perdants). Constantes `WIN_BONUS_INTACT` / `WIN_BONUS_SURVIVOR` (`server/src/services/score.rs`).

### Joueurs éliminés (options de l'hôte, actives par défaut)
Réglées dans le lobby (`PATCH /rooms/{room}/settings` : `reveal_to_eliminated`, `best_question_vote`) :
- **Vue omnisciente** — `GET /games/{game}/reveal?player_id=` : un joueur **éliminé** voit tous les
  binomes, personnages et mots interdits (`403` sinon). Seule exception à « personnages révélés
  seulement à la fin » : l'éliminé ne joue plus.
- **Meilleure question du round** — `POST /games/{game}/question-votes` (`player_id`, `action_id`) :
  un éliminé vote (et peut changer d'avis) pour une question du round en cours ou du précédent,
  ni la sienne ni celle de son binôme (sinon il voterait toujours pour son partenaire). Votes
  publics (`question_votes` dans `GET /games/{game}`, event `QuestionVoted`). À la fin, l'auteur
  de la question la plus votée de chaque round marque **+1** (ex æquo compris) →
  `game_stats.best_questions`, `recap.best_questions`. Voir `server/src/services/score.rs`.

## Comportements particuliers

### Joueur déconnecté : pause, l'hôte décide

La partie est **en pause** quand le joueur qu'elle attend est hors ligne (le joueur dont c'est le
tour, la cible d'une question ou d'une accusation en attente, ou le questionneur qui doit tirer sa
carte). Un autre joueur déconnecté ne bloque rien tant que ce n'est pas à lui. **Seul l'hôte**
(`rooms.created_by`) peut débloquer ; si c'est lui qui est déconnecté, la partie attend son retour.
Le front n'affiche les boutons qu'après 30 s de déconnexion.

**Passer son tour** (`POST /games/{game}/skip-turn`) :

| Ce qu'on attend | `reason` | Effet |
|---|---|---|
| Le joueur dont c'est le tour n'a rien joué | `turn` | Le tour passe, **aucune action** enregistrée |
| La cible d'une accusation n'a pas confirmé | `accusation` | Le **serveur tranche** : nom proposé == vrai personnage (insensible casse / accents / espaces) ; sinon un accusé n'aurait qu'à se déconnecter pour s'en tirer |
| La cible d'une question n'a pas répondu | — | **Refusé (`409`)** : une question attend toujours sa réponse. Attendre le retour du joueur, ou l'exclure |
| Le questionneur doit tirer sa carte (« Je ne sais pas ») | `draw` | La carte est **perdue**, le tour avance (de toute façon automatique au bout de 30 s) |

**Exclure** (`POST /games/{game}/players/{player}/exclude`) :

- L'exclu **et son binôme** sont éliminés ; l'exclu est marqué `is_excluded` et marque **0 point**.
  `eliminated_round` sert à leurs rounds survécus. Un orphelin exclu n'entraîne personne. Révéler le
  partenaire est inhérent à la règle.
- Une question / accusation en attente impliquant un joueur éliminé est **supprimée** : si elle
  venait du joueur courant (toujours en jeu), il rejoue ; si le joueur courant est lui-même
  éliminé, le tour passe au premier joueur actif suivant.
- L'exclusion peut terminer la partie ; l'event `player.excluded` et `recap.exclusions` en rendent compte.

Dans les deux cas, le serveur vérifie dans sa table de présence (en mémoire) que le joueur visé
est bien hors ligne : connecté → `409`. Les mises à jour sont **conditionnelles** : si la cible
répond pile au moment où l'hôte passe son tour, un seul des deux l'emporte et le round n'avance
qu'une fois (implémentation : `server/src/services/play.rs`).

### Spectateurs

Un joueur qui rejoint un salon dont la partie est déjà lancée n'a pas de personnage.
`GET /rooms/{room}` et `POST /rooms/join` renvoient `current_game_id` : le front lui propose de
suivre la partie. Le canal `game.{id}` accepte les membres du salon, et la même règle s'applique
aux réactions. Les spectateurs reçoivent aussi les mots montrés après un tirage de carte.

Les réactions sont toutes en base (`custom_reactions`) : les emojis Unicode d'origine y ont été
repris, l'admin peut en ajouter ou en supprimer. Un emoji Unicode doit être **un seul graphème**.

### Chat commun du hall (`lobby`)

Le chat de la page `/rooms` diffuse sur un **canal public** `lobby` : il doit rester lisible par
un visiteur qui n'a ni créé ni rejoint de salon.

- **Le nom d'auteur est toujours dérivé côté serveur.** `player_id` valide → pseudo du joueur ;
  sinon → `Anonyme#{anon_id}` (identifiant tiré au sort, gardé dans le `localStorage`). Un client
  ne peut donc pas usurper le pseudo d'un joueur.
- `author_name` est **dénormalisé** et `chat_messages.player_id` passe à null si le joueur est
  supprimé : le message garde son auteur d'origine. `is_anonymous` se déduit du préfixe immuable
  de `author_key` (`p:` / `a:`), qui sert aussi au front à reconnaître ses propres messages.
- Les codes de salon cités dans un message sont **confrontés à la table `rooms`** : sans cela,
  n'importe quel mot de 6 caractères (« COUCOU ») passerait pour un code copiable.
- **Durée de vie d'une heure** : purge à chaque lecture et à chaque écriture (aucune tâche
  planifiée), historique limité aux 50 derniers. `LobbyChat.vue` applique la même limite en local.
- **Anti-spam indexé sur l'auteur, pas sur l'IP** (20 messages par minute) : derrière un proxy,
  tous les joueurs du réseau local arrivent avec la même adresse.

### Historique public et classement

**Invariant de sécurité** : l'historique public n'expose que les parties **terminées** (liste,
détail en `404` sinon, classement et fiches joueurs filtrés sur le statut). Publier une partie en
cours révélerait les personnages à ses propres joueurs : il suffirait d'ouvrir un second onglet.
La vue admin, elle, voit tout.

**Clé d'identité du classement : le pseudo.** Les pseudos sont insensibles à la casse et aux
accents (« Killian », « killian » et « Killián » sont un seul joueur, à l'inscription comme au
classement). L'agrégation porte sur `game_stats.player_pseudo`, dénormalisé, qui survit à la
suppression du joueur. Les ex æquo partagent leur rang (1, 2, 2, 4).

**Les parties survivent à leur salon** : quand le dernier joueur quitte un salon, le salon est
supprimé mais ses parties restent dans l'historique et le classement (le code du salon y
apparaît vide). Seul un administrateur peut supprimer une partie, avec un mot de passe dédié
(voir ci-dessous).

### Suppression de parties (administration)

Supprimer une partie (ou un joueur **avec** ses parties) exige, en plus de l'accès admin, un
**mot de passe de suppression** distinct, dont le serveur ne connaît que l'empreinte
(`BINOME_DELETE_PASSWORD_HASH`, PBKDF2-SHA256). Sans empreinte configurée, la suppression de
parties est désactivée. Après 5 mots de passe faux en une minute, les essais sont refusés
(`429`) jusqu'à la fin de la minute.
