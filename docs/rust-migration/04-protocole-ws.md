# Migration Rust — 04. Protocole WebSocket natif

Remplace le protocole Pusher (Laravel Reverb + `laravel-echo` + `pusher-js`). Les **events et
leurs charges utiles sont identiques** (voir `03-events.md`) ; seule l'enveloppe change.
Implémentation : `server/src/realtime/` (hub en mémoire + point d'entrée).

## Connexion

```
GET /binome/ws?token=<jeton de profil>        (upgrade WebSocket, même origine que l'API)
```

- `token` facultatif : c'est le jeton de profil signé (`X-Profile-Token`) ; le serveur en déduit
  le joueur. Sans jeton valide, seul le canal public `lobby` est accessible.
- Depuis le 2026-11-14 (checklist point 5), l'ancien `?player_id=` est **ignoré** : un
  identifiant fourni par le client ne donne plus accès à `player.{id}`, `room.{id}` ni `game.{id}`.
- Pas d'appel `/broadcasting/auth` : l'autorisation se fait à l'abonnement, dans le serveur.

## Messages (JSON texte)

| Sens | Message | Rôle |
|---|---|---|
| client → serveur | `{ "op": "sub", "ch": "game.12" }` | S'abonner |
| client → serveur | `{ "op": "unsub", "ch": "game.12" }` | Se désabonner |
| client → serveur | `{ "op": "ping" }` | Vérifier que la liaison est vivante |
| serveur → client | `{ "op": "sub_ok", "ch", "members": [{ "id", "pseudo" }] }` | Abonnement accepté ; `members` = présents (canaux de présence), sinon `[]` |
| serveur → client | `{ "op": "sub_err", "ch", "status": 403 \| 404 }` | Refusé (403) ou canal inconnu (404) |
| serveur → client | `{ "op": "joining", "ch", "member": { "id", "pseudo" } }` | Un joueur arrive (pas envoyé à lui-même) |
| serveur → client | `{ "op": "leaving", "ch", "member": { "id", "pseudo" } }` | Un joueur part |
| serveur → client | `{ "op": "event", "ch", "event": "round.started", "data": { … } }` | Event métier |
| serveur → client | `{ "op": "pong" }` | Réponse à `ping` |

## Canaux et autorisations

Mêmes règles que l'ancien `routes/channels.php` :

| Canal | Qui | Présence |
|---|---|---|
| `lobby` | tout le monde | non |
| `room.{id}` | membre du salon | oui |
| `game.{id}` | joueur de la partie ou membre du salon (spectateur) | oui |
| `player.{id}` | uniquement `player_id == id` | non |

Présence dédupliquée par joueur : plusieurs onglets = un seul membre ; `joining` au premier
abonnement, `leaving` quand le dernier se ferme. La présence sert aussi au serveur
(« pseudo déjà connecté », « joueur hors ligne » pour passer un tour ou exclure) : lecture en
mémoire, plus d'appel HTTP à Reverb.

## Robustesse

- Le serveur envoie un ping WebSocket toutes les **25 s** et ferme une connexion muette depuis
  **60 s** (téléphone verrouillé, Wi-Fi perdu) : les autres joueurs voient alors `leaving`.
- Le client se reconnecte de lui-même et se réabonne à tous ses canaux ; il signale les
  passages `connected` / `unavailable` pour que la page resynchronise son état (les events
  émis pendant la coupure ne sont pas rejoués, comme avec Pusher).
- Les events sont publiés **après** l'enregistrement en base : un client qui relit l'API à la
  réception d'un event trouve toujours l'état à jour.
- Taille maximale d'un message client : 16 Kio (le client n'envoie que des abonnements).
