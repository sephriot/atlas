use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, NaiveDate, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::config::get_storage_root;
use crate::error::AtlasError;

const MAX_EVENT_JOURNAL_BYTES: usize = 10 * 1024 * 1024;
const DAILY_METRICS_RETENTION_DAYS: i64 = 90;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryCounters {
    pub searches: u64,
    pub zero_result_searches: u64,
    pub results_matched: u64,
    pub gets: u64,
    pub helpful_feedback: u64,
    pub misleading_feedback: u64,
    pub stale_feedback: u64,
    pub missing_feedback: u64,
    #[serde(default)]
    pub commands: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub command_errors: u64,
    #[serde(default)]
    pub sources: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub hooks: std::collections::BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    pub all_time: TelemetryCounters,
    pub daily: std::collections::BTreeMap<String, TelemetryCounters>,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackVerdict {
    Helpful,
    Misleading,
    Stale,
    Missing,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum HookOutcome {
    Injected,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum HookSkipReason {
    AlreadyDone,
    MissingPython,
    ContextFailed,
    NoOrgProject,
    IndexFailed,
    EmptyIndex,
    NoAddendumMatch,
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
    Command {
        timestamp: String,
        command: &'a str,
        ok: bool,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_kind: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hook_event: Option<&'a str>,
    },
    Hook {
        timestamp: String,
        name: &'a str,
        event: &'a str,
        outcome: HookOutcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<HookSkipReason>,
        injected_lines: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum MetricsUpdate<'a> {
    Search {
        total_results: usize,
    },
    Get,
    Feedback(FeedbackVerdict),
    Command {
        name: &'a str,
        ok: bool,
        source: Option<&'a str>,
    },
    Hook {
        name: &'a str,
    },
}

pub fn status() -> Result<TelemetryStatus, AtlasError> {
    Ok(TelemetryStatus {
        enabled: load_config()?.enabled,
        events_path: events_path()?.display().to_string(),
    })
}

pub fn metrics() -> Result<TelemetryMetrics, AtlasError> {
    load_metrics()
}

pub fn set_enabled(enabled: bool) -> Result<TelemetryStatus, AtlasError> {
    let telemetry_dir = telemetry_dir()?;
    fs::create_dir_all(&telemetry_dir)?;
    let config = TelemetryConfig { enabled };
    fs::write(config_path()?, serde_yaml::to_string(&config)?)?;
    status()
}

pub fn clear() -> Result<TelemetryClearResult, AtlasError> {
    let cleared =
        remove_file_if_present(events_path()?)? | remove_file_if_present(metrics_path()?)?;
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
    append_event(
        &TelemetryEvent::Search {
            timestamp: chrono::Utc::now().to_rfc3339(),
            search_id: &search_id,
            total_results,
            result_ids,
        },
        MetricsUpdate::Search { total_results },
    )?;
    Ok(Some(search_id))
}

pub fn record_get(atom_id: &str) -> Result<bool, AtlasError> {
    if !load_config()?.enabled {
        return Ok(false);
    }
    append_event(
        &TelemetryEvent::Get {
            timestamp: chrono::Utc::now().to_rfc3339(),
            atom_id,
        },
        MetricsUpdate::Get,
    )?;
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
    append_event(
        &TelemetryEvent::Feedback {
            timestamp: chrono::Utc::now().to_rfc3339(),
            search_id,
            result_id,
            verdict,
            note,
        },
        MetricsUpdate::Feedback(verdict),
    )?;
    Ok(FeedbackResult { recorded: true })
}

pub fn record_command(
    command: &str,
    ok: bool,
    duration_ms: u64,
    error_kind: Option<&str>,
    source: Option<&str>,
    hook_event: Option<&str>,
) -> Result<bool, AtlasError> {
    if !load_config()?.enabled {
        return Ok(false);
    }
    let source = source.and_then(sanitize_label);
    let hook_event = hook_event.and_then(sanitize_label);
    append_event(
        &TelemetryEvent::Command {
            timestamp: chrono::Utc::now().to_rfc3339(),
            command,
            ok,
            duration_ms,
            error_kind,
            source: source.as_deref(),
            hook_event: hook_event.as_deref(),
        },
        MetricsUpdate::Command {
            name: command,
            ok,
            source: source.as_deref(),
        },
    )?;
    Ok(true)
}

pub fn record_hook(
    name: &str,
    event: &str,
    outcome: HookOutcome,
    reason: Option<HookSkipReason>,
    injected_lines: usize,
) -> Result<FeedbackResult, AtlasError> {
    if !load_config()?.enabled {
        return Ok(FeedbackResult { recorded: false });
    }
    let name = sanitize_label(name).ok_or_else(invalid_hook_label)?;
    let event = sanitize_label(event).ok_or_else(invalid_hook_label)?;
    append_event(
        &TelemetryEvent::Hook {
            timestamp: chrono::Utc::now().to_rfc3339(),
            name: &name,
            event: &event,
            outcome,
            reason,
            injected_lines,
        },
        MetricsUpdate::Hook { name: &name },
    )?;
    Ok(FeedbackResult { recorded: true })
}

fn invalid_hook_label() -> AtlasError {
    AtlasError::Validation(
        "telemetry hook name and event must match [A-Za-z0-9:._-] and be at most 64 characters."
            .to_string(),
    )
}

pub(crate) fn sanitize_label(raw: &str) -> Option<String> {
    if (1..=64).contains(&raw.len())
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-'))
    {
        Some(raw.to_string())
    } else {
        None
    }
}

fn append_event(
    event: &TelemetryEvent<'_>,
    metrics_update: MetricsUpdate<'_>,
) -> Result<(), AtlasError> {
    let path = events_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    serde_json::to_writer(&mut file, event).map_err(|error| {
        AtlasError::Storage(format!("Could not serialize telemetry event: {error}"))
    })?;
    file.write_all(b"\n")?;
    drop(file);
    trim_journal(&path, MAX_EVENT_JOURNAL_BYTES)?;
    save_metrics(metrics_update)?;
    Ok(())
}

fn trim_journal(path: &Path, max_bytes: usize) -> Result<(), AtlasError> {
    let contents = fs::read(path)?;
    if contents.len() <= max_bytes {
        return Ok(());
    }

    let start = contents.len() - max_bytes;
    let retained_start = if contents[start - 1] == b'\n' {
        start
    } else {
        contents[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset + 1)
            .unwrap_or(contents.len())
    };
    fs::write(path, &contents[retained_start..])?;
    Ok(())
}

fn save_metrics(metrics_update: MetricsUpdate<'_>) -> Result<(), AtlasError> {
    let mut metrics = load_metrics()?;
    update_metrics(&mut metrics, Utc::now().date_naive(), metrics_update);
    let path = metrics_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(&metrics)?)?;
    Ok(())
}

fn update_metrics(metrics: &mut TelemetryMetrics, day: NaiveDate, update: MetricsUpdate<'_>) {
    let daily = metrics.daily.entry(day.to_string()).or_default();
    apply_metric_update(&mut metrics.all_time, update);
    apply_metric_update(daily, update);

    let cutoff = (day - Duration::days(DAILY_METRICS_RETENTION_DAYS - 1)).to_string();
    metrics.daily.retain(|date, _| date >= &cutoff);
}

fn apply_metric_update(counters: &mut TelemetryCounters, update: MetricsUpdate<'_>) {
    match update {
        MetricsUpdate::Search { total_results } => {
            counters.searches += 1;
            counters.results_matched += total_results as u64;
            if total_results == 0 {
                counters.zero_result_searches += 1;
            }
        }
        MetricsUpdate::Get => counters.gets += 1,
        MetricsUpdate::Feedback(FeedbackVerdict::Helpful) => counters.helpful_feedback += 1,
        MetricsUpdate::Feedback(FeedbackVerdict::Misleading) => counters.misleading_feedback += 1,
        MetricsUpdate::Feedback(FeedbackVerdict::Stale) => counters.stale_feedback += 1,
        MetricsUpdate::Feedback(FeedbackVerdict::Missing) => counters.missing_feedback += 1,
        MetricsUpdate::Command { name, ok, source } => {
            *counters.commands.entry(name.to_string()).or_default() += 1;
            if !ok {
                counters.command_errors += 1;
            }
            if let Some(source) = source {
                *counters.sources.entry(source.to_string()).or_default() += 1;
            }
        }
        MetricsUpdate::Hook { name } => {
            *counters.hooks.entry(name.to_string()).or_default() += 1;
        }
    }
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

fn load_metrics() -> Result<TelemetryMetrics, AtlasError> {
    let path = metrics_path()?;
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_yaml::from_str(&content)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(TelemetryMetrics::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn remove_file_if_present(path: PathBuf) -> Result<bool, AtlasError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
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

fn metrics_path() -> Result<PathBuf, AtlasError> {
    Ok(telemetry_dir()?.join("metrics.yaml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_journal_retains_the_newest_complete_entries() {
        let path = std::env::temp_dir().join(format!(
            "atlas-telemetry-trim-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::write(&path, b"a\nbbb\ncccc\n").expect("journal should be writable");

        trim_journal(&path, 8).expect("journal should be trimmed");

        assert_eq!(
            fs::read(&path).expect("journal should be readable"),
            b"cccc\n"
        );
        fs::remove_file(path).expect("journal should be removed");
    }

    #[test]
    fn metrics_keep_all_time_totals_after_daily_buckets_expire() {
        let mut metrics = TelemetryMetrics::default();

        update_metrics(
            &mut metrics,
            NaiveDate::from_ymd_opt(2026, 1, 1).expect("date should be valid"),
            MetricsUpdate::Get,
        );
        update_metrics(
            &mut metrics,
            NaiveDate::from_ymd_opt(2026, 8, 27).expect("date should be valid"),
            MetricsUpdate::Search { total_results: 0 },
        );

        assert_eq!(metrics.all_time.gets, 1);
        assert_eq!(metrics.all_time.zero_result_searches, 1);
        assert!(!metrics.daily.contains_key("2026-01-01"));
        assert!(metrics.daily.contains_key("2026-08-27"));
    }

    #[test]
    fn metrics_count_each_feedback_verdict_separately() {
        let mut counters = TelemetryCounters::default();

        for verdict in [
            FeedbackVerdict::Helpful,
            FeedbackVerdict::Misleading,
            FeedbackVerdict::Stale,
            FeedbackVerdict::Missing,
        ] {
            apply_metric_update(&mut counters, MetricsUpdate::Feedback(verdict));
        }

        assert_eq!(counters.helpful_feedback, 1);
        assert_eq!(counters.misleading_feedback, 1);
        assert_eq!(counters.stale_feedback, 1);
        assert_eq!(counters.missing_feedback, 1);
    }

    #[test]
    fn apply_metric_update_counts_commands_errors_sources_and_hooks() {
        let mut counters = TelemetryCounters::default();

        apply_metric_update(
            &mut counters,
            MetricsUpdate::Command {
                name: "index",
                ok: true,
                source: Some("hook:atlas-index"),
            },
        );
        apply_metric_update(
            &mut counters,
            MetricsUpdate::Command {
                name: "index",
                ok: false,
                source: None,
            },
        );
        apply_metric_update(
            &mut counters,
            MetricsUpdate::Hook {
                name: "atlas-index",
            },
        );

        assert_eq!(counters.commands.get("index"), Some(&2));
        assert_eq!(counters.command_errors, 1);
        assert_eq!(counters.sources.get("hook:atlas-index"), Some(&1));
        assert_eq!(counters.hooks.get("atlas-index"), Some(&1));
    }

    #[test]
    fn prechange_counters_deserialize_with_empty_command_maps() {
        let counters: TelemetryCounters = serde_yaml::from_str(
            "searches: 1\nzero_result_searches: 0\nresults_matched: 2\ngets: 3\nhelpful_feedback: 0\nmisleading_feedback: 0\nstale_feedback: 0\nmissing_feedback: 0\n",
        )
        .expect("pre-change counters document should load");

        assert_eq!(counters.searches, 1);
        assert_eq!(counters.gets, 3);
        assert!(counters.commands.is_empty());
        assert_eq!(counters.command_errors, 0);
        assert!(counters.sources.is_empty());
        assert!(counters.hooks.is_empty());
    }

    #[test]
    fn sanitize_label_rejects_spaces_slashes_and_long_strings() {
        assert_eq!(
            sanitize_label("hook:atlas-index").as_deref(),
            Some("hook:atlas-index")
        );
        assert_eq!(sanitize_label("bad source"), None);
        assert_eq!(sanitize_label("hook/atlas-index"), None);
        assert_eq!(sanitize_label(&"a".repeat(65)), None);
    }
}
