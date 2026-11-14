#!/usr/bin/env node
// Génère docs/rust-migration/02-contrat-api.md et 03-events.md à partir du
// contrat observé (tests/e2e/contracts/<cible>.json). Les descriptions sont ici ;
// les formes JSON viennent toujours de la mesure.
//
//   node tests/e2e/render-contract.mjs [rust]

import { readFileSync, writeFileSync } from 'node:fs'

const target = process.argv[2] ?? 'rust'
const contract = JSON.parse(readFileSync(new URL(`./contracts/${target}.json`, import.meta.url), 'utf8'))
const DOCS = new URL('../../docs/rust-migration/', import.meta.url)

// ─── Routes : description + accès ───────────────────────────────────────────
const ROUTES = {
    'POST /api/access/characters': ['Échange le mot de passe admin contre le jeton `X-Characters-Access`', 'public'],
    'GET /api/cosmos': ['Cosmos + `is_enabled` + `universes_count` + `playable_universe_count` + fiche (`description`, `image` = URL ou null) ; `game_modes` proposés', 'public'],
    'POST /api/cosmos': ['Créer un cosmos', 'admin'],
    'GET /api/universes': ['Univers + cosmos + `characters_count`', 'admin'],
    'POST /api/universes': ['Créer un univers', 'admin'],
    'PATCH /api/universes/:universe': ['Changer le cosmos d\'un univers', 'admin'],
    'POST /api/universes/:universe/characters': ['Créer un personnage (multipart, `verif_manual = true`)', 'admin'],
    'GET /api/characters': ['Personnages (sans data URI : `image` = URL)', 'admin'],
    'GET /api/characters/pending': ['Binômes en attente, groupés par univers + niveau', 'admin'],
    'POST /api/characters/validation/accept': ['Valider (`character_ids[]`), image obligatoire', 'admin'],
    'POST /api/characters/validation/reject': ['Refuser → `hidden = true`', 'admin'],
    'POST /api/characters/validation/restore': ['Remettre à valider', 'admin'],
    'PUT /api/characters/:character': ['Modifier un personnage (JSON)', 'admin'],
    'POST /api/characters/:character': ['Modifier un personnage : **POST multipart + `_method=PUT`** (forme utilisée par le front)', 'admin'],
    'PATCH /api/characters/:character/forbidden-words': ['Remplacer les 3 mots interdits', 'admin'],
    'POST /api/characters/:character/image': ['Importer / remplacer l\'image (multipart)', 'admin'],
    'DELETE /api/characters/:character': ['Supprimer (`409` si déjà joué)', 'admin'],
    'GET /api/admin/games': ['Toutes les parties', 'admin'],
    'GET /api/admin/games/:game': ['Détail complet, personnages révélés', 'admin'],
    'DELETE /api/admin/games/:game': ['Supprimer une partie (cascade)', 'admin'],
    'GET /api/admin/players': ['Joueurs + stats + salon + `in_game` + `has_password` / `is_validated` / `admin_role`', 'admin'],
    'PATCH /api/admin/players/:player/admin': ['`{ admin }` : désigner / retirer un administrateur (propriétaire seulement, profil avec mot de passe)', 'admin'],
    'PATCH /api/admin/players/:player/validation': ['`{ validated }` : valider un profil (historique, emojis image)', 'admin'],
    'GET /api/admin/settings': ['Modes de jeu, cosmos et binômes complets, avec leur état', 'admin'],
    'PATCH /api/admin/settings/game-modes/:mode': ['`{ enabled }` (`409` pour le dernier mode actif)', 'admin'],
    'PATCH /api/admin/cosmos/:cosmos': ['`{ is_enabled }` : cosmos proposé aux hôtes', 'admin'],
    'PUT /api/admin/cosmos/:cosmos/sheet': ['Fiche de présentation (multipart ou JSON) : `description` 10–600 car. (rognée), fichier `image` facultatif (remplace), `remove_image=1` pour la retirer', 'admin'],
    'DELETE /api/admin/cosmos/:cosmos/sheet': ['Retirer la fiche (présentation + image)', 'admin'],
    'PATCH /api/admin/binomes': ['`{ universe_id, level_affectation, is_enabled }` : binôme tirable', 'admin'],
    'DELETE /api/admin/players/:player/room': ['Retirer le joueur de son salon', 'admin'],
    'DELETE /api/admin/players/:player': ['Effacer : anonymisé s\'il a joué (parties gardées sous « Joueur supprimé #000123 »), supprimé sinon ; `?with_games=1` (+ `X-Delete-Password`) supprime aussi ses parties ; `409` si déjà anonymisé', 'admin'],
    'DELETE /api/profile': ['Effacer son profil (`{ current_password }` s\'il est protégé) : même effacement que l\'admin, jeton révoqué', 'profil'],
    'GET /api/admin/reactions': ['Toutes les réactions, masquées comprises', 'admin'],
    'POST /api/admin/reactions': ['Ajouter `{name, emoji}` ou `{name, image}`', 'admin'],
    'PATCH /api/admin/reactions/:reaction': ['Renommer / masquer', 'admin'],
    'DELETE /api/admin/reactions/:reaction': ['Supprimer', 'admin'],
    'GET /api/ideas': ['Boîte à idées : favoris puis plus votées ; `has_voted` / `is_mine` / `remaining_today` selon le jeton', 'public'],
    'POST /api/ideas': ['Poster `{player_id, body}` (10–500 car., insultes refusées, 5 par 24 h)', 'player_id · 10/min'],
    'DELETE /api/ideas/:idea': ['Supprimer sa propre idée', 'player_id (auteur)'],
    'PUT /api/ideas/:idea/vote': ['+1 (idempotent ; 409 sur sa propre idée)', 'player_id · 30/min'],
    'DELETE /api/ideas/:idea/vote': ['Retirer son +1 (idempotent)', 'player_id · 30/min'],
    'PATCH /api/admin/ideas/:idea': ['`{ is_pinned }` : favori en tête de liste', 'admin'],
    'DELETE /api/admin/ideas/:idea': ['Supprimer une idée (votes compris)', 'admin'],
    'GET /api/reactions': ['Réactions actives (Unicode puis images ; images pour un profil validé seulement)', 'public'],
    'POST /api/reactions': ['Un joueur crée un emoji image (JPEG carré 64–256 px ≤ 60 ko, 10/pseudo)', 'player_id validé · 5/min'],
    'GET /api/reactions/:reaction/image': ['Image d\'emoji (cache 1 an) ; URL signée `?v=&s=` (sur la création), `404` sans signature valide', 'URL signée'],
    'GET /api/history/games': ['Parties **terminées**', 'profil validé'],
    'GET /api/history/games/:game': ['Détail d\'une partie terminée (`404` sinon)', 'profil validé'],
    'GET /api/history/leaderboard': ['Classement cumulé par pseudo (rangs ex æquo)', 'profil validé'],
    'GET /api/history/players': ['`?pseudo=` totaux + parties (insensible casse/accents)', 'profil validé'],
    'GET /api/chat/messages': ['50 derniers messages de moins d\'1 h', 'public'],
    'POST /api/chat/messages': ['Poster (`player_id` ou `anon_id`) → `chat.message`', 'public · 20/min par auteur'],
    'GET /api/avatars/:avatar': ['Photo JPEG (cache 1 an) ; URL signée `?v=&s=` (sur la dernière modification), `404` sans signature valide ou après changement', 'URL signée'],
    'GET /api/character-images/:character': ['Image de personnage (type d\'origine, cache 1 an)', 'public'],
    'GET /api/cosmos-images/:cosmos': ['Image de la fiche d\'un cosmos (type d\'origine, cache 1 an, URL versionnée `?v=` sur la dernière modification de la fiche)', 'public'],
    'POST /api/players/:player/avatar': ['Enregistrer sa photo (JPEG carré 64–512 px ≤ 200 ko)', 'player_id = :player · 10/min'],
    'DELETE /api/players/:player/avatar': ['Retirer sa photo', 'player_id = :player · 10/min'],
    'POST /api/rooms': ['Créer un salon (`409` si le pseudo est connecté ailleurs)', 'public'],
    'POST /api/rooms/join': ['Rejoindre par code (mêmes `409`)', 'public'],
    'GET /api/rooms/:room': ['État du salon', 'public'],
    'PATCH /api/rooms/:room/settings': ['Mode de jeu + options éliminés', 'hôte'],
    'PATCH /api/rooms/:room/ready': ['Bascule prêt / pas prêt', 'membre'],
    'POST /api/rooms/:room/start': ['Lancer la partie', 'hôte'],
    'DELETE /api/rooms/:room/leave': ['Quitter (`409` si joueur de la partie en cours)', 'membre'],
    'GET /api/games/:game': ['État public (jamais de personnage ni de binôme non découvert)', 'public'],
    'GET /api/games/:game/me': ['Mon personnage + mots interdits', 'player_id'],
    'GET /api/games/:game/recap': ['Récap (`409` tant que non terminée)', 'public'],
    'GET /api/games/:game/reveal': ['Vue omnisciente (éliminés, si l\'option est active)', 'player_id éliminé'],
    'POST /api/games/:game/question-votes': ['Vote meilleure question (éliminés)', 'player_id éliminé'],
    'POST /api/games/:game/character-vote': ['Accepter / changer son personnage', 'player_id'],
    'POST /api/games/:game/reactions': ['Réaction rapide → `reaction.sent`', 'joueur ou spectateur · 30/min'],
    'POST /api/games/:game/skip-turn': ['Passer le tour d\'un joueur déconnecté', 'hôte'],
    'POST /api/games/:game/players/:player/exclude': ['Exclure un joueur déconnecté (+ son binôme)', 'hôte'],
    'POST /api/games/:game/actions/:action/correct-answer': ['Corriger une réponse oui ⇄ non', 'hôte'],
    'POST /api/games/:game/rounds/:round/question': ['Poser une question', 'joueur courant'],
    'POST /api/games/:game/rounds/:round/accusation': ['Accuser (`character_name`)', 'joueur courant'],
    'POST /api/games/:game/rounds/:round/actions/:action/answer': ['Répondre `yes` / `no` / `dont_know`', 'cible'],
    'POST /api/games/:game/rounds/:round/actions/:action/confirm': ['Confirmer / nier une accusation', 'accusé'],
    'POST /api/games/:game/rounds/:round/actions/:action/draw': ['Tirer une carte → `{word, reveal_seconds}` (seul endroit où le mot circule en HTTP)', 'questionneur'],
    'POST /api/games/:game/rounds/:round/actions/:action/draw/decision': ['Montrer (`share: true`) ou garder', 'questionneur'],
}

// ─── Events : canal + déclencheur ───────────────────────────────────────────
const EVENTS = {
    'player.joined': ['`room.{id}`', 'Un joueur rejoint le salon'],
    'player.ready': ['`room.{id}`', 'Bascule prêt'],
    'player.left': ['`room.{id}`', 'Départ (+ `new_host_id` si l\'hôte part)'],
    'room.settings.updated': ['`room.{id}`', 'L\'hôte change les réglages'],
    'game.started': ['`room.{id}` + `game.{id}`', 'Lancement'],
    'player.avatar.updated': ['`room.{id}` de chaque salon du joueur + `game.{id}` en cours', 'Photo modifiée / retirée'],
    'character.vote.cast': ['`game.{id}`', 'Un vote de personnage ; aussi au dernier vote unanime'],
    'characters.redrawn': ['`game.{id}`', 'Tous ont voté, ≥ 1 changement : nouveau tirage'],
    'round.started': ['`game.{id}`', 'Nouveau round (`is_new_round: true`) ou joueur suivant (`false`)'],
    'action.played': ['`game.{id}`', 'Question ou accusation jouée'],
    'answer.given': ['`game.{id}`', 'Réponse (+ `card_draw` si « je ne sais pas » ouvre un tirage)'],
    'answer.corrected': ['`game.{id}`', 'Correction de l\'hôte'],
    'card.drawn': ['`game.{id}`', 'Carte retournée (index seulement)'],
    'card.word.shared': ['`player.{id}` de chaque destinataire **sauf la cible et le questionneur**', 'Mot montré'],
    'card.draw.finished': ['`game.{id}`', 'Tirage clos (montré, gardé ou expiré — indiscernables)'],
    'accusation.confirmed': ['`game.{id}`', 'Accusation tranchée (par l\'accusé ou par le serveur)'],
    'player.eliminated': ['`game.{id}`', 'Accusation juste'],
    'player.excluded': ['`game.{id}`', 'Exclusion par l\'hôte'],
    'turn.skipped': ['`game.{id}`', 'Tour passé par l\'hôte (`reason`: `turn` | `accusation` | `draw`)'],
    'question.voted': ['`game.{id}`', 'Vote meilleure question (tous les votes du round)'],
    'reaction.sent': ['`game.{id}`', 'Réaction rapide (rien n\'est stocké)'],
    'game.ended': ['`game.{id}`', 'Fin de partie : gagnants, stats, révélation complète'],
    'chat.message': ['`lobby` (public)', 'Message du chat du hall'],
}

const shapeBlock = (shape) => '```\n' + Object.entries(shape).map(([p, t]) => `${p.padEnd(48)} ${t}`).join('\n') + '\n```'

// ─── 02 : API ───────────────────────────────────────────────────────────────
const httpKeys = Object.keys(contract).filter((k) => !k.startsWith('event '))
const byRoute = new Map()
for (const key of httpKeys) {
    const [method, path, status, bin] = key.split(' ')
    const route = `${method} ${path}`
    if (!byRoute.has(route)) byRoute.set(route, [])
    byRoute.get(route).push({ status: Number(status), key, binary: Boolean(bin) })
}
const ordered = [...new Set([...Object.keys(ROUTES), ...byRoute.keys()])]

let api = `# Migration Rust — 02. Contrat de l'API REST

> **Généré** par \`tests/e2e/render-contract.mjs\` depuis \`tests/e2e/contracts/${target}.json\`
> (formes observées par le banc E2E, identiques au contrat figé de l'ancien backend Laravel,
> \`contracts/laravel-ref.json\`). Ne pas éditer à la main.

## Conventions communes

- Préfixe : \`/api\` côté serveur ; le navigateur appelle \`/binome/api\` (préfixe ajouté par le proxy).
- Corps JSON ; \`multipart/form-data\` pour les fichiers de personnages. **Method spoofing** :
  \`POST\` + champ \`_method=PUT\` doit être traité comme \`PUT\` (forme utilisée par le front).
- Identité : \`player_id\` dans le corps / la query, **non signé** (modèle de confiance actuel, reproduit tel quel).
- Admin : en-tête \`X-Characters-Access\` = HMAC-SHA256(mot de passe, \`binome-characters-access\`) en hex
  (propriétaire), ou \`X-Profile-Token\` d'un administrateur désigné (\`admin_role = "admin"\`, profil avec mot
  de passe) ; absent ou faux → \`403 {message}\`.
- Erreurs :
  - \`422\` validation → \`{ message, errors: { champ: [messages] } }\` (messages Laravel, en anglais pour les règles génériques, en français pour les messages personnalisés) ;
  - \`409\` règle du jeu → \`{ message }\` ;
  - \`403\` / \`404\` → \`{ message }\` ;
  - \`429\` limite de débit → \`{ message }\`.
  Laravel en mode debug ajoute \`exception / file / line / trace\` : **hors contrat**, à ne pas reproduire.
- Dates : deux formats coexistent selon la route — \`date:iso8601\` = \`2026-11-11T10:00:00+00:00\`,
  \`date:eloquent\` = \`2026-11-11T10:00:00.000000Z\`. Les deux sont à reproduire là où ils apparaissent.
- Booléens : certains champs sortent en \`0\` / \`1\` (\`int\`), ex. \`players[].is_ready\`. À reproduire.
- Images (\`/avatars\`, \`/character-images\`, \`/cosmos-images\`, \`/reactions/:id/image\`) : binaire, \`Cache-Control: public, max-age=31536000, immutable\`,
  \`X-Content-Type-Options: nosniff\`. URLs renvoyées : \`/binome/api/...?v=<updated_at timestamp>\`.
- Limites de débit (par joueur, jamais par IP) : chat 20/min, réactions 30/min, emojis 5/min, avatars 10/min.

## Couverture

${ordered.length} routes ; ${[...byRoute.keys()].length} observées par le banc.
${ordered.filter((r) => !byRoute.has(r)).map((r) => `- **non observée** : \`${r}\``).join('\n') || 'Toutes les routes sont observées.'}

## Routes
`
for (const route of ordered) {
    const [desc, access] = ROUTES[route] ?? ['—', '—']
    api += `\n### \`${route}\`\n\n${desc} — accès : ${access}\n`
    const statuses = (byRoute.get(route) ?? []).sort((a, b) => a.status - b.status)
    if (!statuses.length) { api += '\n_Non observée._\n'; continue }
    api += `\nStatuts observés : ${statuses.map((s) => `\`${s.status}\``).join(', ')}\n`
    for (const s of statuses.filter((x) => x.status < 300)) {
        api += `\nRéponse \`${s.status}\`${s.binary ? ' (binaire)' : ''} :\n\n${shapeBlock(contract[s.key])}\n`
    }
}
writeFileSync(new URL('02-contrat-api.md', DOCS), api)

// ─── 03 : events ────────────────────────────────────────────────────────────
let ev = `# Migration Rust — 03. Événements temps réel

> Formes **générées** par \`tests/e2e/render-contract.mjs\` depuis \`tests/e2e/contracts/${target}.json\`.

## Canaux

| Canal | Type | Qui peut s'abonner (règles de \`routes/channels.php\`) | Membres de présence |
|---|---|---|---|
| \`room.{id}\` | presence | joueur membre du salon | \`{ id, pseudo }\` |
| \`game.{id}\` | presence | joueur de la partie **ou** membre du salon (spectateur) | \`{ id, pseudo }\` |
| \`player.{id}\` | privé | seulement \`player_id == id\` | — |
| \`lobby\` | public | tout le monde, même sans joueur | — |

Présence : un joueur connecté sur plusieurs onglets compte **une fois** ; \`joining\` à la
première connexion, \`leaving\` à la dernière déconnexion. Le front n'interprète un \`leaving\`
qu'après 3 s (rechargement de page).

Garanties observées et vérifiées par le banc :
- \`card.word.shared\` n'est **jamais** reçu par la cible ni renvoyé au questionneur ; le spectateur le reçoit.
- \`card.drawn\` ne contient jamais le mot.
- \`game.started\` ne contient aucun personnage.
- Aucun \`round.started\` n'est diffusé après \`game.ended\`.

Le banc n'observe \`game.started\` que sur \`room.{id}\` : le front ne s'abonne à \`game.{id}\`
qu'après l'avoir reçu. L'event \`binome.discovered\`, écouté par le front, n'est jamais
émis (code mort, non porté).

## Récapitulatif

| Event | Canal | Déclencheur | Observé |
|---|---|---|---|
${Object.entries(EVENTS).map(([name, [ch, why]]) => `| \`${name}\` | ${ch} | ${why} | ${Object.keys(contract).some((k) => k.startsWith(`event ${name} `)) ? '✓' : '—'} |`).join('\n')}

## Payloads
`
for (const [name] of Object.entries(EVENTS)) {
    for (const key of Object.keys(contract).filter((k) => k.startsWith(`event ${name} `))) {
        ev += `\n### \`${name}\` (${key.split(' ').pop()})\n\n${shapeBlock(contract[key])}\n`
    }
}
writeFileSync(new URL('03-events.md', DOCS), ev)
console.log('docs/rust-migration/02-contrat-api.md et 03-events.md générés')
