//! Prometheus PromQL query templates for portsyncd monitoring
//!
//! Provides pre-defined PromQL queries for Grafana dashboards and alerting rules.
//! Queries are organized by category and can be composed dynamically.
//!
//! Features:
//! - Pre-defined queries for common monitoring scenarios
//! - Query builders for dynamic composition
//! - Metric aggregations (rate, sum, avg, max, min)
//! - Time window support
//! - Recording rules for performance optimization
//!
//! Phase 6 Week 5 implementation.

/// PromQL query category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryCategory {
    RecoveryRates,
    SyncDuration,
    ErrorRates,
    HealthMetrics,
    TrendAnalysis,
    Throughput,
    Latency,
    Reliability,
}

/// Time window for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeWindow {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    SixHours,
    OneDay,
}

impl TimeWindow {
    /// Get time window as string suitable for PromQL
    pub fn to_promql_duration(&self) -> &'static str {
        match self {
            TimeWindow::OneMinute => "1m",
            TimeWindow::FiveMinutes => "5m",
            TimeWindow::FifteenMinutes => "15m",
            TimeWindow::OneHour => "1h",
            TimeWindow::SixHours => "6h",
            TimeWindow::OneDay => "1d",
        }
    }
}

/// Built PromQL query string
#[derive(Debug, Clone)]
pub struct PromQLQuery {
    pub query: String,
    pub category: QueryCategory,
    pub description: String,
}

impl PromQLQuery {
    /// Get the query string
    pub fn query_str(&self) -> &str {
        &self.query
    }
}

/// PromQL query builder
pub struct PromQLBuilder;

impl PromQLBuilder {
    // Recovery rate queries (Category: RecoveryRates)

    /// Recovery success rate: successful recoveries / total corruptions
    pub fn recovery_success_rate() -> PromQLQuery {
        PromQLQuery {
            query: "(portsyncd_state_recoveries / (portsyncd_corruptions_detected + 1)) * 100"
                .to_string(),
            category: QueryCategory::RecoveryRates,
            description: "Percentage of corruptions successfully recovered".to_string(),
        }
    }

    /// Corruption rate over time window
    pub fn corruption_rate(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_corruptions_detected[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::RecoveryRates,
            description: format!(
                "Corruption events per second over {} window",
                window.to_promql_duration()
            ),
        }
    }

    /// Unrecovered corruption ratio
    pub fn unrecovered_corruption_ratio() -> PromQLQuery {
        PromQLQuery {
            query:
                "((portsyncd_corruptions_detected - portsyncd_state_recoveries) / (portsyncd_corruptions_detected + 1)) * 100"
                    .to_string(),
            category: QueryCategory::RecoveryRates,
            description: "Percentage of corruptions that are unrecovered".to_string(),
        }
    }

    // Sync duration queries (Category: SyncDuration)

    /// Average initial sync duration
    pub fn avg_sync_duration() -> PromQLQuery {
        PromQLQuery {
            query: "portsyncd_initial_sync_duration_seconds_sum / portsyncd_initial_sync_duration_seconds_count"
                .to_string(),
            category: QueryCategory::SyncDuration,
            description: "Average initial synchronization duration in seconds".to_string(),
        }
    }

    /// Maximum initial sync duration
    pub fn max_sync_duration() -> PromQLQuery {
        PromQLQuery {
            query: "portsyncd_initial_sync_duration_seconds".to_string(),
            category: QueryCategory::SyncDuration,
            description: "Maximum recorded initial sync duration".to_string(),
        }
    }

    /// Sync duration trend
    pub fn sync_duration_trend(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_initial_sync_duration_seconds_sum[{}]) / rate(portsyncd_initial_sync_duration_seconds_count[{}])",
                window.to_promql_duration(),
                window.to_promql_duration()
            ),
            category: QueryCategory::SyncDuration,
            description: format!("Sync duration trend over {}", window.to_promql_duration()),
        }
    }

    // Error rate queries (Category: ErrorRates)

    /// EOIU timeout rate (timeouts / total EOIU signals)
    pub fn eoiu_timeout_rate() -> PromQLQuery {
        PromQLQuery {
            query: "(portsyncd_eoiu_timeouts / (portsyncd_eoiu_detected + 1)) * 100".to_string(),
            category: QueryCategory::ErrorRates,
            description: "EOIU signals that timed out (percentage)".to_string(),
        }
    }

    /// Cold start rate
    pub fn cold_start_rate() -> PromQLQuery {
        PromQLQuery {
            query:
                "(portsyncd_cold_starts / (portsyncd_cold_starts + portsyncd_warm_restarts + 1)) * 100"
                    .to_string(),
            category: QueryCategory::ErrorRates,
            description: "Cold start events as percentage of total restarts".to_string(),
        }
    }

    /// Error events rate over time
    pub fn error_rate(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_corruptions_detected[{}]) + rate(portsyncd_eoiu_timeouts[{}]) + rate(portsyncd_cold_starts[{}])",
                window.to_promql_duration(),
                window.to_promql_duration(),
                window.to_promql_duration()
            ),
            category: QueryCategory::ErrorRates,
            description: format!(
                "Total error rate (corruptions + EOIU timeouts + cold starts) per second over {}",
                window.to_promql_duration()
            ),
        }
    }

    // Health metrics queries (Category: HealthMetrics)

    /// System health score (derived from metrics)
    pub fn health_score() -> PromQLQuery {
        PromQLQuery {
            query: "(100 - ((portsyncd_corruptions_detected * 3) + (portsyncd_eoiu_timeouts / portsyncd_eoiu_detected * 20) + (portsyncd_cold_starts / (portsyncd_cold_starts + portsyncd_warm_restarts) * 20)))".to_string(),
            category: QueryCategory::HealthMetrics,
            description: "Calculated system health score (0-100)".to_string(),
        }
    }

    /// Warm restart success (warm vs cold starts)
    pub fn warm_restart_success_rate() -> PromQLQuery {
        PromQLQuery {
            query:
                "(portsyncd_warm_restarts / (portsyncd_warm_restarts + portsyncd_cold_starts + 1)) * 100"
                    .to_string(),
            category: QueryCategory::HealthMetrics,
            description: "Percentage of restarts that were warm restarts".to_string(),
        }
    }

    /// Overall reliability score
    pub fn reliability_score() -> PromQLQuery {
        PromQLQuery {
            query: "((portsyncd_state_recoveries / (portsyncd_corruptions_detected + 1)) * 50) + ((1 - portsyncd_eoiu_timeouts / (portsyncd_eoiu_detected + 1)) * 50)".to_string(),
            category: QueryCategory::HealthMetrics,
            description: "Reliability score based on recovery and EOIU success rates".to_string(),
        }
    }

    // Trend analysis queries (Category: TrendAnalysis)

    /// Warm restart trend
    pub fn restart_trend(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_warm_restarts[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::TrendAnalysis,
            description: format!(
                "Warm restart rate trend over {}",
                window.to_promql_duration()
            ),
        }
    }

    /// Corruption trend
    pub fn corruption_trend(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "increase(portsyncd_corruptions_detected[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::TrendAnalysis,
            description: format!(
                "Corruption count increase over {}",
                window.to_promql_duration()
            ),
        }
    }

    /// Recovery trend
    pub fn recovery_trend(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_state_recoveries[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::TrendAnalysis,
            description: format!("Recovery rate trend over {}", window.to_promql_duration()),
        }
    }

    // Throughput queries (Category: Throughput)

    /// Event processing throughput
    pub fn event_throughput(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_warm_restarts[{}]) + rate(portsyncd_cold_starts[{}])",
                window.to_promql_duration(),
                window.to_promql_duration()
            ),
            category: QueryCategory::Throughput,
            description: format!(
                "Restart events per second over {}",
                window.to_promql_duration()
            ),
        }
    }

    /// Backup throughput
    pub fn backup_throughput(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_backups_created[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::Throughput,
            description: format!("Backup creation rate over {}", window.to_promql_duration()),
        }
    }

    /// EOIU signal throughput
    pub fn eoiu_throughput(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "rate(portsyncd_eoiu_detected[{}])",
                window.to_promql_duration()
            ),
            category: QueryCategory::Throughput,
            description: format!(
                "EOIU signals per second over {}",
                window.to_promql_duration()
            ),
        }
    }

    // Latency queries (Category: Latency)

    /// P50 sync duration (median, estimated from avg)
    pub fn p50_sync_duration() -> PromQLQuery {
        PromQLQuery {
            query: "portsyncd_initial_sync_duration_seconds_sum / portsyncd_initial_sync_duration_seconds_count"
                .to_string(),
            category: QueryCategory::Latency,
            description: "Estimated P50 (median) sync duration".to_string(),
        }
    }

    /// Sync duration percentile estimation
    pub fn sync_duration_percentile(percentile: u32) -> PromQLQuery {
        let desc = format!("Estimated P{} sync duration", percentile);
        PromQLQuery {
            query: format!(
                "histogram_quantile(0.{}, portsyncd_initial_sync_duration_seconds_bucket)",
                percentile
            ),
            category: QueryCategory::Latency,
            description: desc,
        }
    }

    // Reliability queries (Category: Reliability)

    /// Overall system availability (uptime based on events)
    pub fn system_availability(window: TimeWindow) -> PromQLQuery {
        PromQLQuery {
            query: format!(
                "((portsyncd_warm_restarts - portsyncd_cold_starts) / (portsyncd_warm_restarts + 1)) * 100 over {}",
                window.to_promql_duration()
            ),
            category: QueryCategory::Reliability,
            description: format!(
                "Estimated system availability over {}",
                window.to_promql_duration()
            ),
        }
    }

    /// Backup reliability
    pub fn backup_success_rate() -> PromQLQuery {
        PromQLQuery {
            query:
                "((portsyncd_backups_created - portsyncd_backups_cleaned) / (portsyncd_backups_created + 1)) * 100"
                    .to_string(),
            category: QueryCategory::Reliability,
            description: "Percentage of successfully created backups not cleaned up".to_string(),
        }
    }

    /// Time since last event of each type
    pub fn time_since_last_warm_restart() -> PromQLQuery {
        PromQLQuery {
            query: "time() - portsyncd_last_warm_restart_timestamp".to_string(),
            category: QueryCategory::Reliability,
            description: "Seconds since last warm restart".to_string(),
        }
    }

    /// Get all pre-defined queries for a category
    pub fn queries_for_category(category: QueryCategory) -> Vec<PromQLQuery> {
        match category {
            QueryCategory::RecoveryRates => vec![
                Self::recovery_success_rate(),
                Self::corruption_rate(TimeWindow::FiveMinutes),
                Self::unrecovered_corruption_ratio(),
            ],
            QueryCategory::SyncDuration => vec![
                Self::avg_sync_duration(),
                Self::max_sync_duration(),
                Self::sync_duration_trend(TimeWindow::FiveMinutes),
            ],
            QueryCategory::ErrorRates => vec![
                Self::eoiu_timeout_rate(),
                Self::cold_start_rate(),
                Self::error_rate(TimeWindow::FiveMinutes),
            ],
            QueryCategory::HealthMetrics => vec![
                Self::health_score(),
                Self::warm_restart_success_rate(),
                Self::reliability_score(),
            ],
            QueryCategory::TrendAnalysis => vec![
                Self::restart_trend(TimeWindow::FiveMinutes),
                Self::corruption_trend(TimeWindow::FiveMinutes),
                Self::recovery_trend(TimeWindow::FiveMinutes),
            ],
            QueryCategory::Throughput => vec![
                Self::event_throughput(TimeWindow::FiveMinutes),
                Self::backup_throughput(TimeWindow::FiveMinutes),
                Self::eoiu_throughput(TimeWindow::FiveMinutes),
            ],
            QueryCategory::Latency => vec![
                Self::p50_sync_duration(),
                Self::sync_duration_percentile(95),
                Self::sync_duration_percentile(99),
            ],
            QueryCategory::Reliability => vec![
                Self::system_availability(TimeWindow::OneHour),
                Self::backup_success_rate(),
                Self::time_since_last_warm_restart(),
            ],
        }
    }

    /// Get all pre-defined queries (optimized for batch operations)
    pub fn all_queries() -> Vec<PromQLQuery> {
        // Pre-computed common window for performance
        let five_min = TimeWindow::FiveMinutes;
        let one_hour = TimeWindow::OneHour;

        vec![
            // Recovery rates (3 queries)
            Self::recovery_success_rate(),
            Self::corruption_rate(five_min),
            Self::unrecovered_corruption_ratio(),
            // Sync duration (3 queries)
            Self::avg_sync_duration(),
            Self::max_sync_duration(),
            Self::sync_duration_trend(five_min),
            // Error rates (3 queries)
            Self::eoiu_timeout_rate(),
            Self::cold_start_rate(),
            Self::error_rate(five_min),
            // Health metrics (3 queries)
            Self::health_score(),
            Self::warm_restart_success_rate(),
            Self::reliability_score(),
            // Trends (3 queries)
            Self::restart_trend(five_min),
            Self::corruption_trend(five_min),
            Self::recovery_trend(five_min),
            // Throughput (3 queries)
            Self::event_throughput(five_min),
            Self::backup_throughput(five_min),
            Self::eoiu_throughput(five_min),
            // Latency (3 queries)
            Self::p50_sync_duration(),
            Self::sync_duration_percentile(95),
            Self::sync_duration_percentile(99),
            // Reliability (3 queries)
            Self::system_availability(one_hour),
            Self::backup_success_rate(),
            Self::time_since_last_warm_restart(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_window_to_promql() {
        assert_eq!(TimeWindow::OneMinute.to_promql_duration(), "1m");
        assert_eq!(TimeWindow::FiveMinutes.to_promql_duration(), "5m");
        assert_eq!(TimeWindow::OneHour.to_promql_duration(), "1h");
        assert_eq!(TimeWindow::OneDay.to_promql_duration(), "1d");
    }

    #[test]
    fn test_recovery_success_rate_query() {
        let query = PromQLBuilder::recovery_success_rate();
        assert_eq!(query.category, QueryCategory::RecoveryRates);
        assert!(query.query.contains("portsyncd_state_recoveries"));
        assert!(query.query.contains("portsyncd_corruptions_detected"));
    }

    #[test]
    fn test_corruption_rate_query() {
        let query = PromQLBuilder::corruption_rate(TimeWindow::FiveMinutes);
        assert!(query.query.contains("rate"));
        assert!(query.query.contains("5m"));
    }

    #[test]
    fn test_sync_duration_queries() {
        let avg = PromQLBuilder::avg_sync_duration();
        assert_eq!(avg.category, QueryCategory::SyncDuration);

        let max = PromQLBuilder::max_sync_duration();
        assert_eq!(max.category, QueryCategory::SyncDuration);

        let trend = PromQLBuilder::sync_duration_trend(TimeWindow::OneHour);
        assert!(trend.query.contains("1h"));
    }

    #[test]
    fn test_eoiu_timeout_rate_query() {
        let query = PromQLBuilder::eoiu_timeout_rate();
        assert_eq!(query.category, QueryCategory::ErrorRates);
        assert!(query.query.contains("portsyncd_eoiu_timeouts"));
        assert!(query.query.contains("portsyncd_eoiu_detected"));
    }

    #[test]
    fn test_health_score_query() {
        let query = PromQLBuilder::health_score();
        assert_eq!(query.category, QueryCategory::HealthMetrics);
        assert!(!query.query.is_empty());
    }

    #[test]
    fn test_warm_restart_success_rate_query() {
        let query = PromQLBuilder::warm_restart_success_rate();
        assert_eq!(query.category, QueryCategory::HealthMetrics);
        assert!(query.query.contains("portsyncd_warm_restarts"));
        assert!(query.query.contains("portsyncd_cold_starts"));
    }

    #[test]
    fn test_reliability_score_query() {
        let query = PromQLBuilder::reliability_score();
        assert_eq!(query.category, QueryCategory::HealthMetrics);
    }

    #[test]
    fn test_throughput_queries() {
        let event = PromQLBuilder::event_throughput(TimeWindow::FiveMinutes);
        assert_eq!(event.category, QueryCategory::Throughput);

        let backup = PromQLBuilder::backup_throughput(TimeWindow::FiveMinutes);
        assert_eq!(backup.category, QueryCategory::Throughput);

        let eoiu = PromQLBuilder::eoiu_throughput(TimeWindow::FiveMinutes);
        assert_eq!(eoiu.category, QueryCategory::Throughput);
    }

    #[test]
    fn test_latency_queries() {
        let p50 = PromQLBuilder::p50_sync_duration();
        assert_eq!(p50.category, QueryCategory::Latency);

        let p95 = PromQLBuilder::sync_duration_percentile(95);
        assert!(p95.query.contains("histogram_quantile"));

        let p99 = PromQLBuilder::sync_duration_percentile(99);
        assert!(p99.query.contains("histogram_quantile"));
    }

    #[test]
    fn test_availability_query() {
        let query = PromQLBuilder::system_availability(TimeWindow::OneHour);
        assert_eq!(query.category, QueryCategory::Reliability);
    }

    #[test]
    fn test_backup_success_rate_query() {
        let query = PromQLBuilder::backup_success_rate();
        assert_eq!(query.category, QueryCategory::Reliability);
    }

    #[test]
    fn test_queries_for_category() {
        let recovery_queries = PromQLBuilder::queries_for_category(QueryCategory::RecoveryRates);
        assert!(!recovery_queries.is_empty());

        let health_queries = PromQLBuilder::queries_for_category(QueryCategory::HealthMetrics);
        assert!(!health_queries.is_empty());

        let latency_queries = PromQLBuilder::queries_for_category(QueryCategory::Latency);
        assert!(!latency_queries.is_empty());
    }

    #[test]
    fn test_all_queries() {
        let all_queries = PromQLBuilder::all_queries();
        assert!(
            all_queries.len() >= 23,
            "Should have at least 23 pre-defined queries"
        );
    }

    #[test]
    fn test_promql_query_categories() {
        let all_queries = PromQLBuilder::all_queries();

        let recovery_count = all_queries
            .iter()
            .filter(|q| q.category == QueryCategory::RecoveryRates)
            .count();
        assert!(recovery_count > 0);

        let health_count = all_queries
            .iter()
            .filter(|q| q.category == QueryCategory::HealthMetrics)
            .count();
        assert!(health_count > 0);

        let latency_count = all_queries
            .iter()
            .filter(|q| q.category == QueryCategory::Latency)
            .count();
        assert!(latency_count > 0);
    }
}
