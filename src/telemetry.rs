use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::config::get_storage_root;
use crate::error::AtlasError;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TelemetryConfig {
    enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryStatus {
    pub enabled: bool,
    pub events_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryClearResult {
    pub cleared: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackResult {
    pub recorded: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackVerdict {
    Helpful,
    Misleading,
    Stale,
    Missing,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TelemetryEvent<'a> {
    Search {
        timestamp: String,
        search_id: &'a str,
        total_results: usize,
        result_ids: &'a [String],
    },
    Get {
        timestamp: String,
        atom_id: &'a str,
    },
    Feedback {
        timestamp: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        search_id: Option<&'a str>,
        result_id: Option<&'a str>,
        verdict: FeedbackVerdict,
        note: Option<&'a str>,
    },
}

pub fn status() -> Result<TelemetryStatus, AtlasError> {
    Ok(TelemetryStatus {
        enabled: load_config()?.enabled,
        events_path: events_path()?.display().to_string(),
    })
}

pub fn set_enabled(enabled: bool) -> Result<TelemetryStatus, AtlasError> {
    let telemetry_dir = telemetry_dir()?;
    fs::create_dir_all(&telemetry_dir)?;
    let config = TelemetryConfig { enabled };
    fs::write(config_path()?, serde_yaml::to_string(&config)?)?;
    status()
}

pub fn clear() -> Result<TelemetryClearResult, AtlasError> {
    let events_path = events_path()?;
    let cleared = match fs::remove_file(events_path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    Ok(TelemetryClearResult { cleared })
}

pub fn record_search(
    total_results: usize,
    result_ids: &[String],
) -> Result<Option<String>, AtlasError> {
    if !load_config()?.enabled {
        return Ok(None);
    }

    let search_id = format!(
        "S-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos(),
        std::process::id()
    );
    append_event(&TelemetryEvent::Search {
        timestamp: chrono::Utc::now().to_rfc3339(),
        search_id: &search_id,
        total_results,
        result_ids,
    })?;
    Ok(Some(search_id))
}

pub fn record_get(atom_id: &str) -> Result<bool, AtlasError> {
    if !load_config()?.enabled {
        return Ok(false);
    }
    append_event(&TelemetryEvent::Get {
        timestamp: chrono::Utc::now().to_rfc3339(),
        atom_id,
    })?;
    Ok(true)
}

pub fn record_feedback(
    search_id: Option<&str>,
    result_id: Option<&str>,
    verdict: FeedbackVerdict,
    note: Option<&str>,
) -> Result<FeedbackResult, AtlasError> {
    if !load_config()?.enabled {
        return Ok(FeedbackResult { recorded: false });
    }
    append_event(&TelemetryEvent::Feedback {
        timestamp: chrono::Utc::now().to_rfc3339(),
        search_id,
        result_id,
        verdict,
        note,
    })?;
    Ok(FeedbackResult { recorded: true })
}

fn append_event(event: &TelemetryEvent<'_>) -> Result<(), AtlasError> {
    let path = events_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, event).map_err(|error| {
        AtlasError::Storage(format!("Could not serialize telemetry event: {error}"))
    })?;
    file.write_all(b"\n")?;
    Ok(())
}

fn load_config() -> Result<TelemetryConfig, AtlasError> {
    let path = config_path()?;
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_yaml::from_str(&content)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(TelemetryConfig::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn telemetry_dir() -> Result<PathBuf, AtlasError> {
    Ok(get_storage_root()?.join("telemetry"))
}

fn config_path() -> Result<PathBuf, AtlasError> {
    Ok(telemetry_dir()?.join("config.yaml"))
}

fn events_path() -> Result<PathBuf, AtlasError> {
    Ok(telemetry_dir()?.join("events.jsonl"))
}
