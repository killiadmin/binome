//! Hub temps réel en mémoire : connexions, abonnements, présence, diffusion.
//!
//! Remplace Reverb. La présence est dédupliquée par joueur : plusieurs onglets
//! d'un même joueur = un membre ; `joining` à son premier abonnement,
//! `leaving` quand son dernier se termine (même comportement que Reverb/Pusher).
//! Les messages sont sérialisés une seule fois puis partagés (Arc) entre destinataires.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

/// Event métier à diffuser (après le commit de la transaction qui l'a produit).
#[derive(Debug, Clone)]
pub struct Event {
    pub channels: Vec<String>,
    pub name: &'static str,
    pub data: Value,
}

impl Event {
    pub fn new(channels: Vec<String>, name: &'static str, data: Value) -> Self {
        Self { channels, name, data }
    }
    pub fn on(channel: String, name: &'static str, data: Value) -> Self {
        Self::new(vec![channel], name, data)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Member {
    pub id: i64,
    pub pseudo: String,
}

pub type ConnId = u64;
pub type Outbox = mpsc::UnboundedSender<Arc<str>>;

struct Conn {
    player_id: Option<i64>,
    out: Outbox,
    channels: HashSet<String>,
}

#[derive(Default)]
struct State {
    conns: HashMap<ConnId, Conn>,
    /// canal → connexions abonnées
    subscribers: HashMap<String, HashSet<ConnId>>,
    /// canal de présence → joueur → (nombre de connexions, membre)
    presence: HashMap<String, HashMap<i64, (usize, Member)>>,
}

#[derive(Default)]
pub struct Hub {
    state: Mutex<State>,
    next_id: AtomicU64,
}

fn frame(v: Value) -> Arc<str> {
    Arc::from(v.to_string())
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn register(&self, player_id: Option<i64>, out: Outbox) -> ConnId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.lock().conns.insert(id, Conn { player_id, out, channels: HashSet::new() });
        id
    }

    /// Abonne la connexion. `member` : Some pour un canal de présence.
    /// Renvoie la liste des membres présents (y compris soi) pour la présence.
    pub fn subscribe(&self, conn: ConnId, channel: &str, member: Option<Member>) -> Vec<Member> {
        let mut st = self.lock();
        let Some(c) = st.conns.get_mut(&conn) else { return vec![] };
        if !c.channels.insert(channel.to_string()) {
            // Déjà abonné : on renvoie simplement l'état courant
            return st.presence.get(channel).map(|m| m.values().map(|(_, mb)| mb.clone()).collect()).unwrap_or_default();
        }
        st.subscribers.entry(channel.to_string()).or_default().insert(conn);

        let Some(member) = member else { return vec![] };
        let entry = st.presence.entry(channel.to_string()).or_default();
        let first = match entry.get_mut(&member.id) {
            Some((n, _)) => {
                *n += 1;
                false
            }
            None => {
                entry.insert(member.id, (1, member.clone()));
                true
            }
        };
        let members: Vec<Member> = entry.values().map(|(_, m)| m.clone()).collect();
        if first {
            let msg = frame(json!({ "op": "joining", "ch": channel, "member": member }));
            Self::send_to_channel(&st, channel, &msg, Some(conn), Some(member.id));
        }
        members
    }

    pub fn unsubscribe(&self, conn: ConnId, channel: &str) {
        let mut st = self.lock();
        let Some(c) = st.conns.get_mut(&conn) else { return };
        if !c.channels.remove(channel) {
            return;
        }
        let player = c.player_id;
        Self::detach(&mut st, conn, channel, player);
    }

    /// Fin de connexion : retire tous ses abonnements (et diffuse les `leaving`).
    pub fn unregister(&self, conn: ConnId) {
        let mut st = self.lock();
        let Some(c) = st.conns.remove(&conn) else { return };
        for channel in c.channels {
            Self::detach(&mut st, conn, &channel, c.player_id);
        }
    }

    fn detach(st: &mut State, conn: ConnId, channel: &str, player: Option<i64>) {
        if let Some(set) = st.subscribers.get_mut(channel) {
            set.remove(&conn);
            if set.is_empty() {
                st.subscribers.remove(channel);
            }
        }
        let (Some(pid), Some(members)) = (player, st.presence.get_mut(channel)) else { return };
        let gone = match members.get_mut(&pid) {
            Some((n, _)) if *n > 1 => {
                *n -= 1;
                None
            }
            Some(_) => members.remove(&pid).map(|(_, m)| m),
            None => None,
        };
        if members.is_empty() {
            st.presence.remove(channel);
        }
        if let Some(member) = gone {
            let msg = frame(json!({ "op": "leaving", "ch": channel, "member": member }));
            Self::send_to_channel(st, channel, &msg, None, None);
        }
    }

    fn send_to_channel(st: &State, channel: &str, msg: &Arc<str>, skip_conn: Option<ConnId>, skip_player: Option<i64>) {
        let Some(subs) = st.subscribers.get(channel) else { return };
        for id in subs {
            if Some(*id) == skip_conn {
                continue;
            }
            if let Some(c) = st.conns.get(id) {
                if skip_player.is_some() && c.player_id == skip_player {
                    continue;
                }
                // Envoi non bloquant ; une connexion morte sera nettoyée par sa propre tâche.
                let _ = c.out.send(msg.clone());
            }
        }
    }

    /// Diffuse un event sur chacun de ses canaux.
    pub fn publish(&self, event: &Event) {
        let st = self.lock();
        for channel in &event.channels {
            let msg = frame(json!({ "op": "event", "ch": channel, "event": event.name, "data": event.data }));
            Self::send_to_channel(&st, channel, &msg, None, None);
        }
    }

    pub fn publish_all(&self, events: &[Event]) {
        for e in events {
            self.publish(e);
        }
    }

    pub fn is_online(&self, channel: &str, player_id: i64) -> bool {
        self.lock().presence.get(channel).is_some_and(|m| m.contains_key(&player_id))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(hub: &Hub, player: Option<i64>) -> (ConnId, mpsc::UnboundedReceiver<Arc<str>>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (hub.register(player, tx), rx)
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<Arc<str>>) -> Vec<Value> {
        let mut out = vec![];
        while let Ok(m) = rx.try_recv() {
            out.push(serde_json::from_str(&m).unwrap());
        }
        out
    }

    fn m(id: i64) -> Option<Member> {
        Some(Member { id, pseudo: format!("p{id}") })
    }

    #[test]
    fn presence_is_deduplicated_per_player() {
        let hub = Hub::default();
        let (a, mut ra) = conn(&hub, Some(1));
        let (b1, _rb1) = conn(&hub, Some(2));
        let (b2, _rb2) = conn(&hub, Some(2));
        hub.subscribe(a, "game.1", m(1));
        assert_eq!(hub.subscribe(b1, "game.1", m(2)).len(), 2);
        assert_eq!(drain(&mut ra).iter().filter(|v| v["op"] == "joining").count(), 1);
        hub.subscribe(b2, "game.1", m(2)); // 2e onglet : pas de nouveau joining
        assert!(drain(&mut ra).is_empty());
        hub.unregister(b1); // il reste un onglet : pas de leaving
        assert!(drain(&mut ra).is_empty());
        assert!(hub.is_online("game.1", 2));
        hub.unregister(b2);
        let msgs = drain(&mut ra);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["op"], "leaving");
        assert_eq!(msgs[0]["member"]["id"], 2);
        assert!(hub.is_online("game.1", 1) && !hub.is_online("game.1", 2));
    }

    #[test]
    fn publish_reaches_only_subscribers() {
        let hub = Hub::default();
        let (a, mut ra) = conn(&hub, Some(1));
        let (_b, mut rb) = conn(&hub, Some(2));
        hub.subscribe(a, "player.1", None);
        hub.publish(&Event::on("player.1".into(), "card.word.shared", json!({ "word": "x" })));
        let got = drain(&mut ra);
        assert_eq!(got[0]["event"], "card.word.shared");
        assert_eq!(got[0]["ch"], "player.1");
        assert!(drain(&mut rb).is_empty());
    }
}
