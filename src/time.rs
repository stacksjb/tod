use crate::config::Config;
use chrono::offset::Utc;
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use chrono_tz::Tz;
use regex::Regex;

pub fn now(config: &Config) -> DateTime<Tz> {
    let tz = timezone_from_str(&config.timezone);
    Utc::now().with_timezone(&tz)
}

/// Return today's date in format 2021-09-16
pub fn today_string(config: &Config) -> String {
    now(config).format("%Y-%m-%d").to_string()
}

/// Return today's date in Utc
pub fn today_date(config: &Config) -> NaiveDate {
    now(config).date_naive()
}

pub fn datetime_is_today(datetime: DateTime<Tz>, config: &Config) -> bool {
    date_is_today(datetime.date_naive(), config)
}

pub fn date_is_today(date: NaiveDate, config: &Config) -> bool {
    date.format("%Y-%m-%d").to_string() == today_string(config)
}

pub fn is_date_in_past(date: NaiveDate, config: &Config) -> bool {
    date.signed_duration_since(today_date(config)).num_days() < 0
}

pub fn format_date(date: &NaiveDate, config: &Config) -> String {
    if date_is_today(*date, config) {
        String::from("Today")
    } else {
        date.format("%Y-%m-%d").to_string()
    }
}

pub fn format_datetime(datetime: &DateTime<Tz>, config: &Config) -> String {
    let tz = timezone_from_str(&config.timezone);
    if datetime_is_today(*datetime, config) {
        datetime.with_timezone(&tz).format("%H:%M").to_string()
    } else {
        datetime.with_timezone(&tz).to_string()
    }
}

/// Parse DateTime
pub fn datetime_from_str(str: &str, timezone: Tz) -> Result<DateTime<Tz>, String> {
    let datetime = match str.len() {
        19 => NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S")
            .expect("could not parse DateTime")
            .and_local_timezone(timezone),
        20 => NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%SZ")
            .expect("could not parse DateTime")
            .and_local_timezone(Tz::UTC),
        _ => return Err(format!("cannot parse DateTime: {str}")),
    };

    Ok(datetime.unwrap())
}

pub fn timezone_from_str(timezone_string: &Option<String>) -> Tz {
    match timezone_string {
        None => Tz::UTC,
        Some(string) => string.parse::<Tz>().unwrap(),
    }
}

/// Parse Date
pub fn date_from_str(str: &str, timezone: Tz) -> Result<NaiveDate, String> {
    let date = match str.len() {
        // 19 => NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S")
        //     .expect("could not parse DateTime")
        //     .and_local_timezone(timezone),
        10 => NaiveDate::parse_from_str(str, "%Y-%m-%d").or(Err("could not parse Date"))?,
        19 => NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S")
            .or(Err("could not parse DateTime"))?
            .and_local_timezone(timezone)
            .unwrap()
            .date_naive(),
        20 => NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%SZ")
            .or(Err("could not parse DateTime"))?
            .and_local_timezone(timezone)
            .unwrap()
            .date_naive(),
        _ => return Err(format!("cannot parse NaiveDate, unknown length: {str}")),
    };

    Ok(date)
}

/// Checks if string is a date in format YYYY-MM-DD
pub fn is_date(string: &str) -> bool {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    re.is_match(string)
}

/// Checks if string is a datetime in format YYYY-MM-DD HH:MM
pub fn is_datetime(string: &str) -> bool {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$").unwrap();
    re.is_match(string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_date() {
        assert!(is_date("2022-10-05"));
        assert!(!is_date("22-10-05"));
        assert!(!is_date("2022-10-05 24:02"));
        assert!(!is_date("today"));
    }

    #[test]
    fn test_is_datetime() {
        assert!(!is_datetime("2022-10-05"));
        assert!(!is_datetime("22-10-05"));
        assert!(is_datetime("2022-10-05 24:02"));
        assert!(!is_datetime("today"));
    }
}
