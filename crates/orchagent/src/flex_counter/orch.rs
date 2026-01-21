//! FlexCounterOrch implementation.
//!
//! This is the main orchestrator for flexible counter configuration in SONiC.

use async_trait::async_trait;
use log::{debug, error, info, warn};
use sonic_orch_common::{Consumer, KeyOpFieldsValues, Operation, Orch};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

use super::group::{FlexCounterGroup, FlexCounterGroupMap};
use super::state::{
    parse_index_range, parse_port_list, FlexCounterPgStates, FlexCounterQueueStates,
    PgConfigurations, QueueConfigurations, CREATE_ALL_AVAILABLE_BUFFERS,
};

/// Configuration fields used in FLEX_COUNTER_TABLE.
pub mod fields {
    pub const POLL_INTERVAL: &str = "POLL_INTERVAL";
    pub const STATUS: &str = "FLEX_COUNTER_STATUS";
    pub const STATUS_ENABLE: &str = "enable";
    pub const STATUS_DISABLE: &str = "disable";
    pub const BULK_CHUNK_SIZE: &str = "BULK_CHUNK_SIZE";
    pub const BULK_CHUNK_SIZE_PER_PREFIX: &str = "BULK_CHUNK_SIZE_PER_PREFIX";
}

/// Error type for FlexCounterOrch operations.
#[derive(Debug, thiserror::Error)]
pub enum FlexCounterError {
    #[error("Invalid counter group: {0}")]
    InvalidGroup(String),

    #[error("Invalid poll interval: {0}")]
    InvalidPollInterval(String),

    #[error("Invalid bulk chunk size: {0}")]
    InvalidBulkChunkSize(String),

    #[error("PortsOrch not available")]
    PortsOrchUnavailable,

    #[error("Ports not ready")]
    PortsNotReady,

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for FlexCounterOrch operations.
pub type Result<T> = std::result::Result<T, FlexCounterError>;

/// Configuration for FlexCounterOrch.
#[derive(Debug, Clone)]
pub struct FlexCounterOrchConfig {
    /// Startup delay in seconds before processing counter configurations.
    /// This allows prioritizing data plane configuration during boot.
    pub startup_delay_secs: u64,

    /// Default maximum queue count per port.
    pub default_max_queues: usize,

    /// Default maximum PG count per port.
    pub default_max_pgs: usize,
}

impl Default for FlexCounterOrchConfig {
    fn default() -> Self {
        Self {
            startup_delay_secs: 0,
            default_max_queues: 8,
            default_max_pgs: 8,
        }
    }
}

/// Callback trait for FlexCounterOrch to interact with other Orchs.
///
/// This trait abstracts the dependencies on PortsOrch, IntfsOrch, etc.
/// allowing for testability and gradual migration.
#[async_trait]
pub trait FlexCounterCallbacks: Send + Sync {
    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool;

    /// Returns true if gearbox is enabled.
    fn is_gearbox_enabled(&self) -> bool {
        false
    }

    /// Generates port counter map for the specified group.
    async fn generate_port_counter_map(&self) -> Result<()>;

    /// Generates port buffer drop counter map.
    async fn generate_port_buffer_drop_counter_map(&self) -> Result<()>;

    /// Generates queue map with the given configurations.
    async fn generate_queue_map(&self, configs: &QueueConfigurations) -> Result<()>;

    /// Adds queue flex counters.
    async fn add_queue_flex_counters(&self, configs: &QueueConfigurations) -> Result<()>;

    /// Adds queue watermark flex counters.
    async fn add_queue_watermark_flex_counters(&self, configs: &QueueConfigurations) -> Result<()>;

    /// Generates PG map with the given configurations.
    async fn generate_pg_map(&self, configs: &PgConfigurations) -> Result<()>;

    /// Adds PG flex counters.
    async fn add_pg_flex_counters(&self, configs: &PgConfigurations) -> Result<()>;

    /// Adds PG watermark flex counters.
    async fn add_pg_watermark_flex_counters(&self, configs: &PgConfigurations) -> Result<()>;

    /// Generates WRED port counter map.
    async fn generate_wred_port_counter_map(&self) -> Result<()>;

    /// Adds WRED queue flex counters.
    async fn add_wred_queue_flex_counters(&self, configs: &QueueConfigurations) -> Result<()>;

    /// Flushes all pending counter operations.
    async fn flush_counters(&self) -> Result<()>;

    /// Sets the poll interval for a counter group.
    async fn set_poll_interval(&self, group: &str, interval_ms: u64, gearbox: bool) -> Result<()>;

    /// Enables or disables a counter group.
    async fn set_group_operation(&self, group: &str, enable: bool, gearbox: bool) -> Result<()>;

    /// Sets bulk chunk size for a counter group.
    async fn set_bulk_chunk_size(&self, group: &str, size: Option<u32>) -> Result<()>;
}

/// Internal state for FlexCounterOrch.
#[derive(Debug, Default)]
struct FlexCounterState {
    /// Counter enable/disable states
    port_counter_enabled: bool,
    port_buffer_drop_counter_enabled: bool,
    queue_enabled: bool,
    queue_watermark_enabled: bool,
    pg_enabled: bool,
    pg_watermark_enabled: bool,
    hostif_trap_counter_enabled: bool,
    route_flow_counter_enabled: bool,
    wred_queue_counter_enabled: bool,
    wred_port_counter_enabled: bool,

    /// Whether to create only config DB buffers (vs all available)
    create_only_config_db_buffers: bool,

    /// Groups that have bulk chunk size configured
    groups_with_bulk_chunk_size: HashSet<FlexCounterGroup>,
}

/// FlexCounterOrch - Manages flexible counter configuration.
///
/// This is the Rust implementation of the C++ FlexCounterOrch, providing
/// type-safe counter group management with proper error handling.
pub struct FlexCounterOrch {
    /// Configuration
    config: FlexCounterOrchConfig,

    /// Counter group map
    group_map: FlexCounterGroupMap,

    /// Internal state
    state: FlexCounterState,

    /// Consumer for FLEX_COUNTER_TABLE
    consumer: Consumer,

    /// Startup delay timer
    startup_time: Option<Instant>,

    /// Whether startup delay has expired
    delay_expired: bool,

    /// Callbacks for interacting with other Orchs
    callbacks: Option<Arc<dyn FlexCounterCallbacks>>,

    /// Buffer queue configurations (port -> queue states)
    /// Loaded from CONFIG_DB BUFFER_QUEUE table
    buffer_queue_configs: HashMap<String, Vec<(usize, usize)>>,

    /// Buffer PG configurations (port -> PG states)
    /// Loaded from CONFIG_DB BUFFER_PG table
    buffer_pg_configs: HashMap<String, Vec<(usize, usize)>>,
}

impl FlexCounterOrch {
    /// Creates a new FlexCounterOrch with the given configuration.
    pub fn new(config: FlexCounterOrchConfig) -> Self {
        let startup_time = if config.startup_delay_secs > 0 {
            Some(Instant::now())
        } else {
            None
        };
        let delay_expired = config.startup_delay_secs == 0;

        Self {
            config,
            group_map: FlexCounterGroupMap::new(),
            state: FlexCounterState::default(),
            consumer: Consumer::new(sonic_orch_common::ConsumerConfig::new("FLEX_COUNTER_TABLE")),
            startup_time,
            delay_expired,
            callbacks: None,
            buffer_queue_configs: HashMap::new(),
            buffer_pg_configs: HashMap::new(),
        }
    }

    /// Sets the callbacks for interacting with other Orchs.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn FlexCounterCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns true if port counters are enabled.
    pub fn port_counters_enabled(&self) -> bool {
        self.state.port_counter_enabled
    }

    /// Returns true if port buffer drop counters are enabled.
    pub fn port_buffer_drop_counters_enabled(&self) -> bool {
        self.state.port_buffer_drop_counter_enabled
    }

    /// Returns true if queue counters are enabled.
    pub fn queue_counters_enabled(&self) -> bool {
        self.state.queue_enabled
    }

    /// Returns true if queue watermark counters are enabled.
    pub fn queue_watermark_counters_enabled(&self) -> bool {
        self.state.queue_watermark_enabled
    }

    /// Returns true if PG counters are enabled.
    pub fn pg_counters_enabled(&self) -> bool {
        self.state.pg_enabled
    }

    /// Returns true if PG watermark counters are enabled.
    pub fn pg_watermark_counters_enabled(&self) -> bool {
        self.state.pg_watermark_enabled
    }

    /// Returns true if hostif trap counters are enabled.
    pub fn hostif_trap_counters_enabled(&self) -> bool {
        self.state.hostif_trap_counter_enabled
    }

    /// Returns true if route flow counters are enabled.
    pub fn route_flow_counters_enabled(&self) -> bool {
        self.state.route_flow_counter_enabled
    }

    /// Returns true if WRED queue counters are enabled.
    pub fn wred_queue_counters_enabled(&self) -> bool {
        self.state.wred_queue_counter_enabled
    }

    /// Returns true if WRED port counters are enabled.
    pub fn wred_port_counters_enabled(&self) -> bool {
        self.state.wred_port_counter_enabled
    }

    /// Returns true if only config DB buffers should be created.
    pub fn is_create_only_config_db_buffers(&self) -> bool {
        self.state.create_only_config_db_buffers
    }

    /// Sets whether to create only config DB buffers.
    pub fn set_create_only_config_db_buffers(&mut self, value: bool) {
        if self.state.create_only_config_db_buffers != value {
            info!(
                "create_only_config_db_buffers changed from {} to {}",
                self.state.create_only_config_db_buffers, value
            );
            self.state.create_only_config_db_buffers = value;
        }
    }

    /// Loads buffer queue configuration from a key-value entry.
    ///
    /// Key format: "port_names:queue_range" (e.g., "Ethernet0,Ethernet4:0-3")
    pub fn load_buffer_queue_config(&mut self, key: &str) {
        if let Some((ports_str, range_str)) = key.rsplit_once(':') {
            if let Some((start, end)) = parse_index_range(range_str) {
                for port in parse_port_list(ports_str) {
                    self.buffer_queue_configs
                        .entry(port.to_string())
                        .or_default()
                        .push((start, end));
                }
            } else {
                warn!("Invalid queue range in buffer config: {}", key);
            }
        } else {
            warn!("Invalid buffer queue config key format: {}", key);
        }
    }

    /// Loads buffer PG configuration from a key-value entry.
    ///
    /// Key format: "port_names:pg_range" (e.g., "Ethernet0:0-7")
    pub fn load_buffer_pg_config(&mut self, key: &str) {
        if let Some((ports_str, range_str)) = key.rsplit_once(':') {
            if let Some((start, end)) = parse_index_range(range_str) {
                for port in parse_port_list(ports_str) {
                    self.buffer_pg_configs
                        .entry(port.to_string())
                        .or_default()
                        .push((start, end));
                }
            } else {
                warn!("Invalid PG range in buffer config: {}", key);
            }
        } else {
            warn!("Invalid buffer PG config key format: {}", key);
        }
    }

    /// Gets queue configurations for counter registration.
    ///
    /// If `create_only_config_db_buffers` is false, returns a special
    /// marker indicating all queues should have counters enabled.
    pub fn get_queue_configurations(&self) -> QueueConfigurations {
        if !self.state.create_only_config_db_buffers {
            let mut configs = QueueConfigurations::new();
            configs.insert(
                CREATE_ALL_AVAILABLE_BUFFERS.to_string(),
                FlexCounterQueueStates::default(),
            );
            return configs;
        }

        let mut configs = QueueConfigurations::new();
        for (port, ranges) in &self.buffer_queue_configs {
            let mut states = FlexCounterQueueStates::new(self.config.default_max_queues);
            for &(start, end) in ranges {
                states.enable_queue_counters(start, end);
            }
            configs.insert(port.clone(), states);
        }
        configs
    }

    /// Gets PG configurations for counter registration.
    pub fn get_pg_configurations(&self) -> PgConfigurations {
        if !self.state.create_only_config_db_buffers {
            let mut configs = PgConfigurations::new();
            configs.insert(
                CREATE_ALL_AVAILABLE_BUFFERS.to_string(),
                FlexCounterPgStates::default(),
            );
            return configs;
        }

        let mut configs = PgConfigurations::new();
        for (port, ranges) in &self.buffer_pg_configs {
            let mut states = FlexCounterPgStates::new(self.config.default_max_pgs);
            for &(start, end) in ranges {
                states.enable_pg_counters(start, end);
            }
            configs.insert(port.clone(), states);
        }
        configs
    }

    /// Checks if the startup delay has expired.
    fn check_delay_expired(&mut self) -> bool {
        if self.delay_expired {
            return true;
        }

        if let Some(startup_time) = self.startup_time {
            let delay = Duration::from_secs(self.config.startup_delay_secs);
            if startup_time.elapsed() >= delay {
                info!("FlexCounterOrch startup delay expired");
                self.delay_expired = true;
                return true;
            }
        }

        false
    }

    /// Processes a SET operation for a counter group.
    async fn process_set(
        &mut self,
        group: FlexCounterGroup,
        fields: &HashMap<String, String>,
        callbacks: &dyn FlexCounterCallbacks,
    ) -> Result<()> {
        let sai_group = group.sai_group_name();
        let gearbox = callbacks.is_gearbox_enabled() && group.supports_gearbox();

        // Process POLL_INTERVAL
        if let Some(interval_str) = fields.get(fields::POLL_INTERVAL) {
            let interval_ms: u64 = interval_str
                .parse()
                .map_err(|_| FlexCounterError::InvalidPollInterval(interval_str.clone()))?;

            debug!("Setting poll interval for {} to {} ms", group, interval_ms);
            callbacks
                .set_poll_interval(sai_group, interval_ms, false)
                .await?;

            if gearbox {
                callbacks
                    .set_poll_interval(sai_group, interval_ms, true)
                    .await?;
            }

            self.group_map.set_poll_interval(group, interval_ms);
        }

        // Process STATUS (enable/disable)
        if let Some(status) = fields.get(fields::STATUS) {
            let enable = status == fields::STATUS_ENABLE;
            info!("{} counter group {}", if enable { "Enabling" } else { "Disabling" }, group);

            // Generate counter maps based on group type
            if enable {
                self.enable_counter_group(group, callbacks).await?;
            }

            // Set the operation (enable/disable polling)
            callbacks
                .set_group_operation(sai_group, enable, false)
                .await?;

            if gearbox {
                callbacks
                    .set_group_operation(sai_group, enable, true)
                    .await?;
            }

            self.group_map.set_enabled(group, enable);
            self.update_state_flags(group, enable);

            // Flush counters
            callbacks.flush_counters().await?;
        }

        // Process BULK_CHUNK_SIZE
        let bulk_size = fields.get(fields::BULK_CHUNK_SIZE);
        let bulk_size_per_prefix = fields.get(fields::BULK_CHUNK_SIZE_PER_PREFIX);

        if bulk_size.is_some() || bulk_size_per_prefix.is_some() {
            let size = bulk_size
                .or(bulk_size_per_prefix)
                .and_then(|s| s.parse().ok());

            if let Some(size) = size {
                debug!("Setting bulk chunk size for {} to {}", group, size);
                callbacks.set_bulk_chunk_size(sai_group, Some(size)).await?;
                self.group_map.set_bulk_chunk_size(group, size);
                self.state.groups_with_bulk_chunk_size.insert(group);
            }
        } else if self.state.groups_with_bulk_chunk_size.contains(&group) {
            // Clear bulk chunk size if it was previously set but now removed
            debug!("Clearing bulk chunk size for {}", group);
            callbacks.set_bulk_chunk_size(sai_group, None).await?;
            self.group_map.clear_bulk_chunk_size(group);
            self.state.groups_with_bulk_chunk_size.remove(&group);
        }

        Ok(())
    }

    /// Enables a counter group by generating the appropriate counter maps.
    async fn enable_counter_group(
        &self,
        group: FlexCounterGroup,
        callbacks: &dyn FlexCounterCallbacks,
    ) -> Result<()> {
        match group {
            FlexCounterGroup::Port | FlexCounterGroup::PortRates => {
                callbacks.generate_port_counter_map().await?;
            }
            FlexCounterGroup::PortBufferDrop => {
                callbacks.generate_port_buffer_drop_counter_map().await?;
            }
            FlexCounterGroup::Queue => {
                let configs = self.get_queue_configurations();
                callbacks.generate_queue_map(&configs).await?;
                callbacks.add_queue_flex_counters(&configs).await?;
            }
            FlexCounterGroup::QueueWatermark => {
                let configs = self.get_queue_configurations();
                callbacks.generate_queue_map(&configs).await?;
                callbacks.add_queue_watermark_flex_counters(&configs).await?;
            }
            FlexCounterGroup::WredEcnQueue => {
                let configs = self.get_queue_configurations();
                callbacks.generate_queue_map(&configs).await?;
                callbacks.add_wred_queue_flex_counters(&configs).await?;
            }
            FlexCounterGroup::PgDrop => {
                let configs = self.get_pg_configurations();
                callbacks.generate_pg_map(&configs).await?;
                callbacks.add_pg_flex_counters(&configs).await?;
            }
            FlexCounterGroup::PgWatermark => {
                let configs = self.get_pg_configurations();
                callbacks.generate_pg_map(&configs).await?;
                callbacks.add_pg_watermark_flex_counters(&configs).await?;
            }
            FlexCounterGroup::WredEcnPort => {
                callbacks.generate_wred_port_counter_map().await?;
            }
            // Other groups are handled by their respective Orchs
            // via callbacks or direct implementation
            _ => {
                debug!("Counter group {} handled externally", group);
            }
        }

        Ok(())
    }

    /// Updates internal state flags based on group enable/disable.
    fn update_state_flags(&mut self, group: FlexCounterGroup, enable: bool) {
        match group {
            FlexCounterGroup::Port | FlexCounterGroup::PortRates => {
                self.state.port_counter_enabled = enable;
            }
            FlexCounterGroup::PortBufferDrop => {
                self.state.port_buffer_drop_counter_enabled = enable;
            }
            FlexCounterGroup::Queue => {
                self.state.queue_enabled = enable;
            }
            FlexCounterGroup::QueueWatermark => {
                self.state.queue_watermark_enabled = enable;
            }
            FlexCounterGroup::PgDrop => {
                self.state.pg_enabled = enable;
            }
            FlexCounterGroup::PgWatermark => {
                self.state.pg_watermark_enabled = enable;
            }
            FlexCounterGroup::FlowCntTrap => {
                self.state.hostif_trap_counter_enabled = enable;
            }
            FlexCounterGroup::FlowCntRoute => {
                self.state.route_flow_counter_enabled = enable;
            }
            FlexCounterGroup::WredEcnQueue => {
                self.state.wred_queue_counter_enabled = enable;
            }
            FlexCounterGroup::WredEcnPort => {
                self.state.wred_port_counter_enabled = enable;
            }
            _ => {}
        }
    }

    /// Adds a task to the consumer for processing.
    pub fn add_task(&mut self, key: String, op: Operation, fields: HashMap<String, String>) {
        let fvs: Vec<(String, String)> = fields.into_iter().collect();
        self.consumer.add_to_sync(vec![KeyOpFieldsValues::new(key, op, fvs)]);
    }
}

#[async_trait]
impl Orch for FlexCounterOrch {
    fn name(&self) -> &str {
        "FlexCounterOrch"
    }

    fn priority(&self) -> i32 {
        // FlexCounterOrch has low priority - data plane config is more important
        100
    }

    async fn do_task(&mut self) {
        // Check startup delay
        if !self.check_delay_expired() {
            debug!("FlexCounterOrch waiting for startup delay");
            return;
        }

        // Check if callbacks are available and clone Arc for later use
        let callbacks = match &self.callbacks {
            Some(cb) => cb.clone(),
            None => {
                debug!("FlexCounterOrch: callbacks not set");
                return;
            }
        };

        // Check if ports are ready
        if !callbacks.all_ports_ready() {
            debug!("FlexCounterOrch waiting for ports to be ready");
            return;
        }

        // Process pending tasks
        let tasks = self.consumer.drain();

        for task in tasks {
            match task.op {
                Operation::Set => {
                    // Parse the counter group from the key
                    let group = match task.key.parse::<FlexCounterGroup>() {
                        Ok(g) => g,
                        Err(e) => {
                            warn!("Invalid flex counter group: {}", e);
                            continue;
                        }
                    };

                    // Convert field values to HashMap
                    let fields: HashMap<String, String> = task.fvs.into_iter().collect();

                    if let Err(e) = self.process_set(group, &fields, callbacks.as_ref()).await {
                        error!("Failed to process {} SET: {}", group, e);
                    }
                }
                Operation::Del => {
                    // Handle DEL by disabling the group
                    if let Ok(group) = task.key.parse::<FlexCounterGroup>() {
                        info!("Disabling counter group {} (deleted)", group);
                        self.group_map.set_enabled(group, false);
                        self.update_state_flags(group, false);
                    }
                }
            }
        }
    }

    fn has_pending_tasks(&self) -> bool {
        self.consumer.has_pending()
    }

    fn bake(&mut self) -> bool {
        // FlexCounters are not data plane configuration required during warm restart
        true
    }

    fn dump_pending_tasks(&self) -> Vec<String> {
        self.consumer
            .peek()
            .map(|t| format!("{}:{:?}", t.key, t.op))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flex_counter_orch_new() {
        let config = FlexCounterOrchConfig::default();
        let orch = FlexCounterOrch::new(config);

        assert_eq!(orch.name(), "FlexCounterOrch");
        assert!(!orch.port_counters_enabled());
        assert!(!orch.queue_counters_enabled());
    }

    #[test]
    fn test_flex_counter_orch_with_delay() {
        let config = FlexCounterOrchConfig {
            startup_delay_secs: 30,
            ..Default::default()
        };
        let orch = FlexCounterOrch::new(config);

        assert!(orch.startup_time.is_some());
        assert!(!orch.delay_expired);
    }

    #[test]
    fn test_load_buffer_queue_config() {
        let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());

        orch.load_buffer_queue_config("Ethernet0:0-3");
        orch.load_buffer_queue_config("Ethernet0:4-7");
        orch.load_buffer_queue_config("Ethernet4,Ethernet8:0-3");

        assert_eq!(orch.buffer_queue_configs.get("Ethernet0").unwrap().len(), 2);
        assert!(orch.buffer_queue_configs.contains_key("Ethernet4"));
        assert!(orch.buffer_queue_configs.contains_key("Ethernet8"));
    }

    #[test]
    fn test_get_queue_configurations_all_buffers() {
        let orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());

        let configs = orch.get_queue_configurations();
        assert!(configs.contains_key(CREATE_ALL_AVAILABLE_BUFFERS));
    }

    #[test]
    fn test_get_queue_configurations_selective() {
        let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
        orch.set_create_only_config_db_buffers(true);
        orch.load_buffer_queue_config("Ethernet0:0-3");

        let configs = orch.get_queue_configurations();
        assert!(!configs.contains_key(CREATE_ALL_AVAILABLE_BUFFERS));

        let eth0_states = configs.get("Ethernet0").unwrap();
        assert!(eth0_states.is_queue_counter_enabled(0));
        assert!(eth0_states.is_queue_counter_enabled(3));
        assert!(!eth0_states.is_queue_counter_enabled(4));
    }

    #[test]
    fn test_update_state_flags() {
        let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());

        orch.update_state_flags(FlexCounterGroup::Port, true);
        assert!(orch.port_counters_enabled());

        orch.update_state_flags(FlexCounterGroup::Queue, true);
        assert!(orch.queue_counters_enabled());

        orch.update_state_flags(FlexCounterGroup::Port, false);
        assert!(!orch.port_counters_enabled());
    }

    #[test]
    fn test_bake() {
        let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
        // bake() always returns true for FlexCounterOrch
        assert!(orch.bake());
    }

    #[test]
    fn test_add_task() {
        let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());

        let mut fields = HashMap::new();
        fields.insert(fields::STATUS.to_string(), fields::STATUS_ENABLE.to_string());

        orch.add_task("PORT".to_string(), Operation::Set, fields);

        assert!(orch.has_pending_tasks());
        assert_eq!(orch.dump_pending_tasks().len(), 1);
    }
}
