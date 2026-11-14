//! Point d'entrée WebSocket : `GET {prefix}/ws?token=<jeton de profil>` (facultatif :
//! sans jeton valide, un visiteur n'a accès qu'au canal public `lobby`). Le joueur
//! est déduit du jeton, jamais d'un identifiant fourni par le client.
//!
//! Protocole (JSON texte) :
//!   client → { "op": "sub", "ch": "game.12" } | { "op": "unsub", "ch": … } | { "op": "ping" }
//!   serveur → { "op": "sub_ok", "ch", "members": [{id, pseudo}] }   (présence : membres présents)
//!             { "op": "sub_err", "ch", "status": 403 }
//!             { "op": "joining" | "leaving", "ch", "member": {id, pseudo} }
//!             { "op": "event", "ch", "event": "round.started", "data": {…} }
//!             { "op": "pong" }
//! Le serveur envoie un ping WebSocket toutes les 25 s et coupe une connexion
//! muette depuis 60 s ; le client se reconnecte et se réabonne.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::db::Q;
use crate::realtime::hub::Member;
use crate::state::AppState;

const PING_EVERY: Duration = Duration::from_secs(25);
const SILENCE_LIMIT: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

pub async fn upgrade(ws: WebSocketUpgrade, Query(q): Query<WsQuery>, State(state): State<AppState>) -> Response {
    // Joueur prouvé par son jeton de profil ; sans jeton valide : visiteur (lobby seul).
    let player_id = match q.token {
        Some(token) => {
            let secret = state.profile_secret.clone();
            state.db.read(move |c| crate::services::profile::authenticate_with(c, &secret, Some(&token))).await.ok().flatten().map(|p| p.id)
        }
        None => None,
    };
    // Tampons réduits (défaut : 128 Kio chacun par connexion) : les messages de
    // Binome font quelques Ko, et chaque téléphone connecté coûterait sinon ~110 Kio.
    ws.max_message_size(16 * 1024)
        .read_buffer_size(4 * 1024)
        .write_buffer_size(4 * 1024)
        .on_upgrade(move |socket| session(socket, state, player_id))
}

/// Décision d'autorisation pour un canal.
enum Access {
    Granted(Option<Member>),
    Denied,
}

/// Mêmes règles que l'ancien `routes/channels.php`.
async fn authorize(state: &AppState, player_id: Option<i64>, channel: String) -> Access {
    if channel == "lobby" {
        return Access::Granted(None);
    }
    let Some(pid) = player_id else { return Access::Denied };
    let (kind, id) = match channel.split_once('.') {
        Some((k, id)) => match id.parse::<i64>() {
            Ok(id) => (k.to_string(), id),
            Err(_) => return Access::Denied,
        },
        None => return Access::Denied,
    };
    if kind == "player" {
        return if pid == id { Access::Granted(None) } else { Access::Denied };
    }
    let result = state
        .db
        .read(move |c| {
            let Some(pseudo) = c.opt("SELECT pseudo FROM players WHERE id = ?1", [pid], |r| r.get::<_, String>(0))? else {
                return Ok(None);
            };
            let allowed = match kind.as_str() {
                "room" => c.exists("SELECT 1 FROM room_player WHERE room_id = ?1 AND player_id = ?2", [id, pid])?,
                "game" => {
                    // Joueur de la partie, ou membre du salon (spectateur)
                    c.exists(
                        "SELECT 1 FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id WHERE b.game_id = ?1 AND bp.player_id = ?2",
                        [id, pid],
                    )? || c.exists(
                        "SELECT 1 FROM games g JOIN room_player rp ON rp.room_id = g.room_id WHERE g.id = ?1 AND rp.player_id = ?2",
                        [id, pid],
                    )?
                }
                _ => false,
            };
            Ok(allowed.then_some(Member { id: pid, pseudo }))
        })
        .await;
    match result {
        Ok(Some(member)) => Access::Granted(Some(member)),
        _ => Access::Denied,
    }
}

fn is_valid_channel(ch: &str) -> bool {
    ch == "lobby"
        || ch
            .split_once('.')
            .is_some_and(|(k, id)| matches!(k, "room" | "game" | "player") && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
}

async fn session(socket: WebSocket, state: AppState, player_id: Option<i64>) {
    let (mut sink, mut stream) = socket.split();
    let (out, mut inbox) = mpsc::unbounded_channel::<Arc<str>>();
    let conn = state.hub.register(player_id, out.clone());

    // Écriture : messages du hub + pings périodiques
    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING_EVERY);
        ping.tick().await;
        loop {
            tokio::select! {
                msg = inbox.recv() => match msg {
                    Some(text) => if sink.send(Message::Text(text.as_ref().into())).await.is_err() { break },
                    None => break,
                },
                _ = ping.tick() => if sink.send(Message::Ping(Vec::new().into())).await.is_err() { break },
            }
        }
        let _ = sink.close().await;
    });

    let reply = |v: Value| {
        let _ = out.send(Arc::from(v.to_string()));
    };

    let mut last_seen = Instant::now();
    loop {
        let next = tokio::time::timeout(SILENCE_LIMIT.saturating_sub(last_seen.elapsed()), stream.next()).await;
        let msg = match next {
            Ok(Some(Ok(m))) => m,
            _ => break, // silence prolongé, erreur réseau ou fermeture
        };
        last_seen = Instant::now();
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue, // pong / ping / binaire
        };
        let Ok(req) = serde_json::from_str::<Value>(&text) else { continue };
        let ch = req["ch"].as_str().unwrap_or_default().to_string();
        match req["op"].as_str() {
            Some("ping") => reply(json!({ "op": "pong" })),
            Some("sub") if is_valid_channel(&ch) => match authorize(&state, player_id, ch.clone()).await {
                Access::Granted(member) => {
                    let is_presence = member.is_some();
                    let members = state.hub.subscribe(conn, &ch, member);
                    let members = if is_presence { json!(members) } else { json!([]) };
                    reply(json!({ "op": "sub_ok", "ch": ch, "members": members }));
                }
                Access::Denied => reply(json!({ "op": "sub_err", "ch": ch, "status": 403 })),
            },
            Some("sub") => reply(json!({ "op": "sub_err", "ch": ch, "status": 404 })),
            Some("unsub") => state.hub.unsubscribe(conn, &ch),
            _ => {}
        }
    }

    state.hub.unregister(conn);
    writer.abort();
}
