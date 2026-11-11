//! Minuteries en mémoire (remplacent la queue Laravel et le worker `queue:listen`).
//!
//! Un tirage de carte non décidé est clos à son échéance. Au démarrage, les
//! tirages encore ouverts sont réarmés depuis `card_draw_deadline` : un
//! redémarrage du serveur ne bloque aucun tour.

use std::time::Duration;

use crate::clock;
use crate::db::Q;
use crate::services::play;
use crate::state::AppState;

pub fn schedule_card_expiry(state: AppState, action_id: i64, deadline: &str) {
    let wait = clock::parse(deadline)
        .map(|d| (d - clock::now()).whole_milliseconds().max(0) as u64)
        .unwrap_or(0);
    tokio::spawn(async move {
        // Une seconde de marge : le dernier clic à l'échéance passe avant la minuterie
        tokio::time::sleep(Duration::from_millis(wait + 1000)).await;
        let result = state
            .db
            .write(move |tx| {
                let mut events = vec![];
                play::expire_card_draw(tx, action_id, &mut events)?;
                Ok(events)
            })
            .await;
        match result {
            Ok(events) => state.publish(&events),
            Err(e) => tracing::error!("expiration du tirage {action_id} : {e:?}"),
        }
    });
}

/// Réarme les tirages ouverts (après un redémarrage).
pub async fn rearm(state: &AppState) {
    let pending = state
        .db
        .read(|c| {
            c.all(
                "SELECT id, card_draw_deadline FROM actions WHERE card_draw_status IN ('pending', 'drawn') AND card_draw_deadline IS NOT NULL",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
        })
        .await
        .unwrap_or_default();
    if !pending.is_empty() {
        tracing::info!("{} tirage(s) de carte réarmé(s)", pending.len());
    }
    for (id, deadline) in pending {
        schedule_card_expiry(state.clone(), id, &deadline);
    }
}
