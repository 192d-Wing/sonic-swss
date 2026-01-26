//! REST API handlers for neighsyncd
//!
//! Provides HTTP/REST endpoints using Axum web framework.
//! Implements JSON serialization for API responses.

use crate::grpc_api::{
    ApiError, ConfigInfo, HealthInfo, NeighborInfo, NeighsyncService, QueryParams, StatsInfo,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// JSON response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Success flag
    pub success: bool,
    /// Response data
    pub data: Option<T>,
    /// Error info if failed
    pub error: Option<ApiErrorResponse>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create error response
    pub fn error(error: ApiErrorResponse) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

/// Error response structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiErrorResponse {
    /// Error code
    pub code: u32,
    /// Error message
    pub message: String,
    /// Optional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl From<ApiError> for ApiErrorResponse {
    fn from(error: ApiError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    }
}

/// Query parameters for neighbor list
#[derive(Debug, Deserialize)]
pub struct ListNeighborsQuery {
    /// Filter by interface
    pub interface: Option<String>,
    /// Filter by state
    pub state: Option<String>,
    /// Filter by family
    pub family: Option<String>,
    /// Result limit
    pub limit: Option<u32>,
}

impl From<ListNeighborsQuery> for QueryParams {
    fn from(q: ListNeighborsQuery) -> Self {
        QueryParams {
            interface: q.interface,
            state: q.state,
            family: q.family,
            limit: q.limit,
        }
    }
}

/// REST API service wrapper
pub struct RestApiService {
    /// Inner service implementation
    service: Arc<dyn NeighsyncService>,
}

impl RestApiService {
    /// Create new REST API service
    pub fn new(service: Arc<dyn NeighsyncService>) -> Self {
        Self { service }
    }

    /// List neighbors
    pub async fn list_neighbors(
        &self,
        query: ListNeighborsQuery,
    ) -> std::result::Result<ApiResponse<Vec<NeighborInfo>>, ApiErrorResponse> {
        match self.service.get_neighbors(&query.into()) {
            Ok(neighbors) => Ok(ApiResponse::success(neighbors)),
            Err(e) => {
                error!(error = %e, "Failed to list neighbors");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to list neighbors".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Get specific neighbor
    pub async fn get_neighbor(
        &self,
        ip_address: String,
    ) -> std::result::Result<ApiResponse<NeighborInfo>, ApiErrorResponse> {
        match self.service.get_neighbor(&ip_address) {
            Ok(neighbor) => Ok(ApiResponse::success(neighbor)),
            Err(e) => {
                error!(ip = %ip_address, error = %e, "Failed to get neighbor");
                Err(ApiErrorResponse {
                    code: 404,
                    message: format!("Neighbor {} not found", ip_address),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Add neighbor
    pub async fn add_neighbor(
        &self,
        neighbor: NeighborInfo,
    ) -> std::result::Result<ApiResponse<String>, ApiErrorResponse> {
        match self.service.add_neighbor(&neighbor) {
            Ok(()) => {
                info!(ip = %neighbor.ip_address, "Neighbor added via REST API");
                Ok(ApiResponse::success(format!(
                    "Neighbor {} added successfully",
                    neighbor.ip_address
                )))
            }
            Err(e) => {
                error!(error = %e, "Failed to add neighbor");
                Err(ApiErrorResponse {
                    code: 400,
                    message: "Failed to add neighbor".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Delete neighbor
    pub async fn delete_neighbor(
        &self,
        ip_address: String,
    ) -> std::result::Result<ApiResponse<String>, ApiErrorResponse> {
        match self.service.delete_neighbor(&ip_address) {
            Ok(()) => {
                info!(ip = %ip_address, "Neighbor deleted via REST API");
                Ok(ApiResponse::success(format!(
                    "Neighbor {} deleted successfully",
                    ip_address
                )))
            }
            Err(e) => {
                error!(error = %e, "Failed to delete neighbor");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to delete neighbor".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Get health status
    pub async fn get_health(
        &self,
    ) -> std::result::Result<ApiResponse<HealthInfo>, ApiErrorResponse> {
        match self.service.get_health() {
            Ok(health) => Ok(ApiResponse::success(health)),
            Err(e) => {
                error!(error = %e, "Failed to get health");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to get health status".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Get statistics
    pub async fn get_stats(&self) -> std::result::Result<ApiResponse<StatsInfo>, ApiErrorResponse> {
        match self.service.get_stats() {
            Ok(stats) => Ok(ApiResponse::success(stats)),
            Err(e) => {
                error!(error = %e, "Failed to get stats");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to get statistics".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Get configuration
    pub async fn get_config(
        &self,
    ) -> std::result::Result<ApiResponse<ConfigInfo>, ApiErrorResponse> {
        match self.service.get_config() {
            Ok(config) => Ok(ApiResponse::success(config)),
            Err(e) => {
                error!(error = %e, "Failed to get config");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to get configuration".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Update configuration
    pub async fn update_config(
        &self,
        config: ConfigInfo,
    ) -> std::result::Result<ApiResponse<String>, ApiErrorResponse> {
        match self.service.update_config(&config) {
            Ok(()) => {
                info!("Configuration updated via REST API");
                Ok(ApiResponse::success(
                    "Configuration updated successfully".to_string(),
                ))
            }
            Err(e) => {
                error!(error = %e, "Failed to update config");
                Err(ApiErrorResponse {
                    code: 400,
                    message: "Failed to update configuration".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Restart daemon
    pub async fn restart(&self) -> std::result::Result<ApiResponse<String>, ApiErrorResponse> {
        match self.service.restart() {
            Ok(()) => {
                info!("Restart requested via REST API");
                Ok(ApiResponse::success("Restart command accepted".to_string()))
            }
            Err(e) => {
                error!(error = %e, "Failed to restart");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to restart daemon".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }

    /// Get daemon status
    pub async fn get_status(&self) -> std::result::Result<ApiResponse<String>, ApiErrorResponse> {
        match self.service.get_status() {
            Ok(status) => Ok(ApiResponse::success(status)),
            Err(e) => {
                error!(error = %e, "Failed to get status");
                Err(ApiErrorResponse {
                    code: 500,
                    message: "Failed to get daemon status".to_string(),
                    details: Some(e.to_string()),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc_api::MockNeighsyncService;

    #[tokio::test]
    async fn test_list_neighbors() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service);

        let query = ListNeighborsQuery {
            interface: None,
            state: None,
            family: None,
            limit: None,
        };

        let result = api.list_neighbors(query).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_add_neighbor() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service);

        let neighbor = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        let result = api.add_neighbor(neighbor).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_get_neighbor() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service.clone());

        let neighbor = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        api.add_neighbor(neighbor.clone()).await.unwrap();
        let result = api.get_neighbor("fe80::1".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_neighbor() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service);

        let neighbor = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        api.add_neighbor(neighbor).await.unwrap();
        let result = api.delete_neighbor("fe80::1".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_health() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service);

        let result = api.get_health().await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let service = Arc::new(MockNeighsyncService::new());
        let api = RestApiService::new(service);

        let result = api.get_stats().await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
    }

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let error = ApiErrorResponse {
            code: 404,
            message: "Not found".to_string(),
            details: None,
        };
        let response: ApiResponse<String> = ApiResponse::error(error);
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[test]
    fn test_query_params_conversion() {
        let query = ListNeighborsQuery {
            interface: Some("eth0".to_string()),
            state: Some("reachable".to_string()),
            family: None,
            limit: Some(100),
        };

        let params: QueryParams = query.into();
        assert_eq!(params.interface, Some("eth0".to_string()));
        assert_eq!(params.state, Some("reachable".to_string()));
        assert_eq!(params.limit, Some(100));
    }
}
