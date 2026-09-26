//! Datas das operações: leitura de `AAAA-MM-DD`, "hoje" no Brasil e formatos de exibição.

use time::{Date, Month, OffsetDateTime, UtcOffset};

/// Data de hoje no horário de Brasília (UTC-3, sem horário de verão).
pub fn today() -> Date {
    let brasilia = UtcOffset::from_hms(-3, 0, 0).expect("offset válido");
    OffsetDateTime::now_utc().to_offset(brasilia).date()
}

/// Lê uma data `AAAA-MM-DD` (o formato do `<input type="date">`).
pub fn parse_iso(text: &str) -> Option<Date> {
    let mut parts = text.trim().splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

/// `2026-09-26`
pub fn iso(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// `26/09/2026`
pub fn br(date: Date) -> String {
    format!(
        "{:02}/{:02}/{:04}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// `26/09/26`
pub fn br_short(date: Date) -> String {
    format!(
        "{:02}/{:02}/{:02}",
        date.day(),
        u8::from(date.month()),
        date.year() % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_dates() {
        let date = parse_iso("2026-02-28").unwrap();
        assert_eq!(iso(date), "2026-02-28");
        assert_eq!(br(date), "28/02/2026");
        assert_eq!(br_short(date), "28/02/26");
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(parse_iso("2026-02-30").is_none());
        assert!(parse_iso("26/09/2026").is_none());
        assert!(parse_iso("").is_none());
    }
}
