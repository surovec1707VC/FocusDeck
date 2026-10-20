//! Quick add: one line → a task.
//!
//! `Отчёт по продажам завтра !2 #Работа/Отчёты *еженедельно`
//! - `!1`…`!3` (или `!`, `!!`, `!!!`) — приоритет;
//! - `#категория` или `#категория/подкатегория`;
//! - `сегодня`, `завтра`, `послезавтра`, `@пн`…`@вс`, `@2026-10-25`, `@25.10` — срок;
//! - `*ежедневно`, `*будни`, `*еженедельно`, `*ежемесячно` (или `*daily` и т.д.) — повтор.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::Serialize;

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct QuickTask {
    pub title: String,
    pub priority: Option<i64>,
    pub due: Option<NaiveDate>,
    pub category: Option<String>,
    pub recurrence: Option<&'static str>,
}

fn weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "пн" | "mon" => Weekday::Mon,
        "вт" | "tue" => Weekday::Tue,
        "ср" | "wed" => Weekday::Wed,
        "чт" | "thu" => Weekday::Thu,
        "пт" | "fri" => Weekday::Fri,
        "сб" | "sat" => Weekday::Sat,
        "вс" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

/// Next date with this weekday, today excluded.
fn next_weekday(today: NaiveDate, wd: Weekday) -> NaiveDate {
    let diff = (wd.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64 + 7) % 7;
    today + Duration::days(if diff == 0 { 7 } else { diff })
}

fn date(token: &str, today: NaiveDate) -> Option<NaiveDate> {
    if let Ok(d) = NaiveDate::parse_from_str(token, "%Y-%m-%d") {
        return Some(d);
    }
    // ДД.ММ → this year, or next year if already past.
    let (d, m) = token.split_once('.')?;
    let (d, m): (u32, u32) = (d.parse().ok()?, m.parse().ok()?);
    let this = NaiveDate::from_ymd_opt(today.year(), m, d)?;
    if this < today { NaiveDate::from_ymd_opt(today.year() + 1, m, d) } else { Some(this) }
}

pub fn parse(text: &str, today: NaiveDate) -> QuickTask {
    let mut q = QuickTask::default();
    let mut words = Vec::new();
    for word in text.split_whitespace() {
        let lower = word.to_lowercase();
        let handled = match lower.as_str() {
            "сегодня" | "today" => {
                q.due = Some(today);
                true
            }
            "завтра" | "tomorrow" => {
                q.due = Some(today + Duration::days(1));
                true
            }
            "послезавтра" => {
                q.due = Some(today + Duration::days(2));
                true
            }
            "!" | "!!" | "!!!" => {
                q.priority = Some(lower.len() as i64);
                true
            }
            "!1" | "!2" | "!3" => {
                q.priority = lower[1..].parse().ok();
                true
            }
            "*ежедневно" | "*daily" => {
                q.recurrence = Some("daily");
                true
            }
            "*будни" | "*weekdays" => {
                q.recurrence = Some("weekdays");
                true
            }
            "*еженедельно" | "*weekly" => {
                q.recurrence = Some("weekly");
                true
            }
            "*ежемесячно" | "*monthly" => {
                q.recurrence = Some("monthly");
                true
            }
            w if w.len() > 1 && w.starts_with('#') && q.category.is_none() => {
                q.category = Some(word[1..].trim_matches('/').to_string());
                true
            }
            w if w.starts_with('@') => {
                let rest = &w[1..];
                match weekday(rest).map(|wd| next_weekday(today, wd)).or_else(|| date(rest, today)) {
                    Some(d) => {
                        q.due = Some(d);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        };
        if !handled {
            words.push(word);
        }
    }
    q.title = words.join(" ");
    // A repeating task needs a date to repeat from.
    if q.recurrence.is_some() && q.due.is_none() {
        q.due = Some(today);
    }
    q
}

/// Date of the next occurrence of a repeating task.
pub fn next_due(from: NaiveDate, recurrence: &str) -> Option<NaiveDate> {
    match recurrence {
        "daily" => Some(from + Duration::days(1)),
        "weekdays" => {
            let mut d = from + Duration::days(1);
            while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
                d += Duration::days(1);
            }
            Some(d)
        }
        "weekly" => Some(from + Duration::days(7)),
        "monthly" => from.checked_add_months(chrono::Months::new(1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parses_tokens() {
        let today = d("2026-10-20"); // вторник
        let q = parse("Отчёт по продажам завтра !2 #Работа/Отчёты *еженедельно", today);
        assert_eq!(q.title, "Отчёт по продажам");
        assert_eq!(q.due, Some(d("2026-10-21")));
        assert_eq!(q.priority, Some(2));
        assert_eq!(q.category.as_deref(), Some("Работа/Отчёты"));
        assert_eq!(q.recurrence, Some("weekly"));

        assert_eq!(parse("Звонок @пт", today).due, Some(d("2026-10-23")));
        assert_eq!(parse("Звонок @вт", today).due, Some(d("2026-10-27")), "same weekday = next week");
        assert_eq!(parse("x @01.02", today).due, Some(d("2027-02-01")));
        assert_eq!(parse("x @2026-12-31 !!!", today).priority, Some(3));
        assert_eq!(parse("Зарядка *daily", today).due, Some(today));
        // Unknown tokens stay in the title.
        assert_eq!(parse("письмо @boss #", today).title, "письмо @boss #");
    }

    #[test]
    fn recurrence() {
        assert_eq!(next_due(d("2026-10-23"), "weekdays"), Some(d("2026-10-26")));
        assert_eq!(next_due(d("2026-01-31"), "monthly"), Some(d("2026-02-28")));
        assert_eq!(next_due(d("2026-10-20"), "none"), None);
    }
}
