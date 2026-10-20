//! Weather from open-meteo.com (no key, no account). The only network request
//! FocusDeck makes, and only with the `network.weather` permission. The last
//! forecast is kept in `widget_cache` and shown when there is no network.

use std::sync::OnceLock;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use super::data::Fetched;
use crate::automation::engine::parse_ts;
use crate::error::{AppError, AppResult};
use crate::permissions::{self, PERM_NETWORK_WEATHER};

const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("FocusDeck/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client")
    })
}

async fn cached(pool: &SqlitePool, key: &str) -> AppResult<Option<(Value, String)>> {
    let row: Option<(String, String)> = sqlx::query_as("SELECT data_json, fetched_at FROM widget_cache WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    row.map(|(d, at)| Ok((serde_json::from_str(&d)?, at))).transpose()
}

async fn store(pool: &SqlitePool, key: &str, data: &Value) -> AppResult<String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO widget_cache (key, data_json, fetched_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET data_json = excluded.data_json, fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(data.to_string())
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(now)
}

async fn fetch(city: &str, fahrenheit: bool) -> anyhow::Result<Value> {
    let geo: Value = client()
        .get(GEOCODING_URL)
        .query(&[("name", city), ("count", "1"), ("language", "ru"), ("format", "json")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let place = geo
        .get("results")
        .and_then(|r| r.get(0))
        .ok_or_else(|| anyhow::anyhow!("город «{city}» не найден"))?;
    let (lat, lon) = (
        place["latitude"].as_f64().unwrap_or_default(),
        place["longitude"].as_f64().unwrap_or_default(),
    );
    let forecast: Value = client()
        .get(FORECAST_URL)
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("current", "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m,is_day".into()),
            ("daily", "weather_code,temperature_2m_max,temperature_2m_min".into()),
            ("timezone", "auto".into()),
            ("forecast_days", "3".into()),
            ("wind_speed_unit", "ms".into()),
            ("temperature_unit", if fahrenheit { "fahrenheit" } else { "celsius" }.into()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let c = &forecast["current"];
    let d = &forecast["daily"];
    let days = d["time"]
        .as_array()
        .map(|t| {
            t.iter()
                .enumerate()
                .map(|(i, date)| {
                    json!({
                        "date": date,
                        "code": d["weather_code"][i],
                        "max": d["temperature_2m_max"][i],
                        "min": d["temperature_2m_min"][i],
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "city": place["name"],
        "country": place["country"],
        "units": if fahrenheit { "fahrenheit" } else { "celsius" },
        "current": {
            "temperature": c["temperature_2m"],
            "apparent": c["apparent_temperature"],
            "humidity": c["relative_humidity_2m"],
            "wind": c["wind_speed_10m"],
            "code": c["weather_code"],
            "is_day": c["is_day"].as_i64() == Some(1),
        },
        "daily": days,
    }))
}

/// Fresh cache → cache; otherwise the network; on failure → cache with the error.
pub async fn get(pool: &SqlitePool, settings: &Value, max_age_secs: u32, force: bool) -> AppResult<Fetched> {
    let city = settings.get("city").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if city.is_empty() || city.chars().count() > 100 {
        return Err(AppError::Invalid("укажите город в настройках виджета «Погода»".into()));
    }
    let fahrenheit = settings.get("units").and_then(Value::as_str) == Some("fahrenheit");
    let key = format!("weather:{}:{}", city.to_lowercase(), if fahrenheit { "f" } else { "c" });
    let cache = cached(pool, &key).await?;

    let age = cache
        .as_ref()
        .and_then(|(_, at)| parse_ts(at))
        .map(|at| (Utc::now() - at).num_seconds())
        .unwrap_or(i64::MAX);
    if !force && age < max_age_secs as i64 {
        let (data, at) = cache.unwrap();
        return Ok(Fetched { data, fetched_at: Some(at), from_cache: true, error: None });
    }

    let failure = |error: String| match &cache {
        Some((data, at)) => Fetched { data: data.clone(), fetched_at: Some(at.clone()), from_cache: true, error: Some(error) },
        None => Fetched { data: Value::Null, fetched_at: None, from_cache: false, error: Some(error) },
    };
    if !permissions::is_granted(pool, PERM_NETWORK_WEATHER).await? {
        return Ok(failure("нет разрешения «Сеть: прогноз погоды» (страница «Разрешения»)".into()));
    }
    match fetch(&city, fahrenheit).await {
        Ok(data) => {
            let at = store(pool, &key, &data).await?;
            Ok(Fetched { data, fetched_at: Some(at), from_cache: false, error: None })
        }
        Err(e) => {
            tracing::info!("weather: {e}");
            let msg = if e.downcast_ref::<reqwest::Error>().is_some() {
                "нет связи с сервером погоды".to_string()
            } else {
                e.to_string()
            };
            Ok(failure(msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn offline_without_permission_uses_cache() {
        let pool = crate::db::test_pool().await;
        crate::permissions::seed(&pool).await.unwrap();
        let settings = json!({ "city": "Москва", "units": "celsius" });

        let empty = get(&pool, &settings, 60, false).await.unwrap();
        assert!(empty.data.is_null() && empty.error.is_some());

        store(&pool, "weather:москва:c", &json!({ "city": "Москва" })).await.unwrap();
        let fresh = get(&pool, &settings, 600, false).await.unwrap();
        assert!(fresh.from_cache && fresh.error.is_none());
        // Stale cache + no permission: cached data with an explanation.
        let stale = get(&pool, &settings, 600, true).await.unwrap();
        assert_eq!(stale.data["city"], "Москва");
        assert!(stale.error.unwrap().contains("разрешения"));

        assert!(get(&pool, &json!({ "city": " " }), 60, false).await.is_err());
    }
}
