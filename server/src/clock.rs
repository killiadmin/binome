//! Dates : stockage texte UTC « YYYY-MM-DD HH:MM:SS » (précision de MySQL TIMESTAMP)
//! et les deux formats de sortie du contrat :
//!   iso8601  → 2026-11-11T10:00:00+00:00      (Carbon::toIso8601String)
//!   eloquent → 2026-11-11T10:00:00.000000Z    (sérialisation JSON d'un modèle)

use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

const DB: &[FormatItem<'static>] = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
const ISO: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]+00:00");
const ELOQUENT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].000000Z");

pub fn now() -> OffsetDateTime {
    let n = OffsetDateTime::now_utc();
    n.replace_nanosecond(0).unwrap_or(n)
}

/// Horodatage « maintenant » au format de stockage.
pub fn now_db() -> String {
    to_db(now())
}

pub fn to_db(t: OffsetDateTime) -> String {
    t.format(DB).expect("format de date")
}

pub fn in_seconds_db(seconds: i64) -> String {
    to_db(now() + Duration::seconds(seconds))
}

pub fn parse(db: &str) -> Option<OffsetDateTime> {
    PrimitiveDateTime::parse(db, DB).ok().map(|p| p.assume_utc())
}

pub fn iso(db: Option<&str>) -> Option<String> {
    db.and_then(parse).map(|t| t.format(ISO).expect("format iso"))
}

pub fn eloquent(db: Option<&str>) -> Option<String> {
    db.and_then(parse).map(|t| t.format(ELOQUENT).expect("format eloquent"))
}

/// Horodatage Unix (version des URLs d'images `?v=`), vide si absent.
pub fn unix(db: Option<&str>) -> String {
    db.and_then(parse).map(|t| t.unix_timestamp().to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let db = "2026-11-11 10:05:09";
        assert_eq!(iso(Some(db)).unwrap(), "2026-11-11T10:05:09+00:00");
        assert_eq!(eloquent(Some(db)).unwrap(), "2026-11-11T10:05:09.000000Z");
        assert_eq!(unix(Some(db)), "1794391509");
        assert_eq!(iso(None), None);
    }
}
