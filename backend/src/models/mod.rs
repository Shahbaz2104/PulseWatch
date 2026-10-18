//! Shared domain models and query/report window helpers.

pub mod check_result;
pub mod incident;
pub mod monitor;
pub mod reports;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::AppError;

/// Half-open reporting window `[start, end)` in UTC.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct ReportWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Outcome of parsing window query parameters.
pub const WINDOW_PRESETS: [(&str, i64); 3] = [("24h", 24), ("7d", 24 * 7), ("30d", 24 * 30)];

impl ReportWindow {
    pub fn last_hours(hours: i64) -> Self {
        let end = Utc::now();
        Self {
            start: end - chrono::Duration::hours(hours),
            end,
        }
    }

    /// Parse `window` (+ optional `from`/`to`) query parameters.
    ///
    /// Presets: `24h`, `7d`, `30d`, `custom` (requires `from`, optional `to`).
    /// Defaults to `24h` when omitted. Rejects inverted or invalid ranges.
    pub fn from_query(
        window: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Self, AppError> {
        let window = window.unwrap_or("24h");
        if let Some((_, hours)) = WINDOW_PRESETS.iter().find(|(name, _)| *name == window) {
            return Ok(Self::last_hours(*hours));
        }
        match window {
            "custom" => {
                let start = parse_rfc3339(
                    "from",
                    from.ok_or_else(|| {
                        AppError::validation("from", "required when window=custom")
                    })?,
                )?;
                let end = match to {
                    Some(value) => parse_rfc3339("to", value)?,
                    None => Utc::now(),
                };
                if start >= end {
                    return Err(AppError::validation(
                        "from",
                        "must be earlier than the window end",
                    ));
                }
                if end > Utc::now() + chrono::Duration::minutes(5) {
                    return Err(AppError::validation("to", "cannot be in the future"));
                }
                Ok(Self { start, end })
            }
            other => Err(AppError::validation(
                "window",
                format!("unknown window '{other}', expected one of 24h, 7d, 30d, custom"),
            )),
        }
    }

    pub fn duration_hours(&self) -> f64 {
        (self.end - self.start).num_milliseconds() as f64 / 3_600_000.0
    }
}

pub fn parse_rfc3339(field: &str, value: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| AppError::validation(field, "must be an RFC3339 timestamp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_parse() {
        for (preset, hours) in WINDOW_PRESETS {
            let window = ReportWindow::from_query(Some(preset), None, None).unwrap();
            let expected = chrono::Duration::hours(hours);
            let delta = (window.end - window.start) - expected;
            assert_eq!(delta, chrono::Duration::zero(), "preset {preset}");
        }
    }

    #[test]
    fn default_is_24h() {
        let window = ReportWindow::from_query(None, None, None).unwrap();
        assert_eq!((window.end - window.start).num_hours(), 24);
    }

    #[test]
    fn custom_window_requires_from() {
        let err = ReportWindow::from_query(Some("custom"), None, None).unwrap_err();
        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn custom_window_accepts_forward_range() {
        let to = Utc::now();
        let from = to - chrono::Duration::hours(1);
        let window = ReportWindow::from_query(
            Some("custom"),
            Some(&from.to_rfc3339()),
            Some(&to.to_rfc3339()),
        )
        .unwrap();
        assert_eq!(window.start, from);
        assert_eq!(window.end, to);
    }

    #[test]
    fn custom_window_rejects_backward_range() {
        let from = Utc::now();
        let to = from - chrono::Duration::hours(1);
        let err = ReportWindow::from_query(
            Some("custom"),
            Some(&from.to_rfc3339()),
            Some(&to.to_rfc3339()),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn unknown_window_rejected() {
        assert!(ReportWindow::from_query(Some("1y"), None, None).is_err());
    }

    #[test]
    fn invalid_timestamp_rejected() {
        let err = ReportWindow::from_query(Some("custom"), Some("yesterday"), None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
