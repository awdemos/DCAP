use crate::{
    error::{NegotiationError, Result},
    model::{AgentInfo, AgentType, PaymentMethod},
    AgentId,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub agent_type: AgentType,
    pub name: String,
    pub endpoint: String,
    pub public_key: String,
    pub payment_methods: Vec<PaymentMethod>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub category: Option<String>,
    pub min_reputation: Option<u32>,
    pub payment_methods: Option<Vec<PaymentMethod>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub agents: Vec<AgentInfo>,
    pub total_count: u32,
}

pub struct DiscoveryService {
    endpoint: String,
    client: Client,
}

impl DiscoveryService {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: Client::new(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn register_agent(&self, agent_info: AgentInfo) -> Result<()> {
        // Notify remote discovery service if available
        if !self.endpoint.is_empty() {
            let request = RegisterRequest {
                agent_type: agent_info.agent_type,
                name: agent_info.name,
                endpoint: agent_info.endpoint,
                public_key: agent_info.public_key,
                payment_methods: agent_info.payment_methods,
            };

            let response = self.client
                .post(&format!("{}/register", self.endpoint))
                .json(&request)
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(NegotiationError::Network(response.error_for_status().unwrap_err()));
            }
        }

        Ok(())
    }

    pub async fn search_sellers(&self, request: SearchRequest) -> Result<Vec<AgentInfo>> {
        let mut agents = Vec::new();

        // Query remote discovery service if available
        if !self.endpoint.is_empty() {
            if let Ok(remote_agents) = self.search_remote_sellers(&request).await {
                agents = remote_agents;
            }
        }

        // Apply filters
        if let Some(min_reputation) = request.min_reputation {
            agents.retain(|agent| agent.reputation_score >= min_reputation);
        }

        if let Some(payment_methods) = &request.payment_methods {
            agents.retain(|agent| {
                agent.payment_methods.iter().any(|pm| payment_methods.contains(pm))
            });
        }

        if let Some(category) = &request.category {
            // This would require filtering by product categories
            // For now, we'll just return all sellers
        }

        Ok(agents)
    }

    pub async fn get_agent(&self, agent_id: AgentId) -> Result<AgentInfo> {
        // Try remote discovery service
        if !self.endpoint.is_empty() {
            let response = self.client
                .get(&format!("{}/agents/{}", self.endpoint, agent_id))
                .send()
                .await?;

            if response.status().is_success() {
                return response.json().await.map_err(Into::into);
            }
        }

        Err(NegotiationError::AgentNotFound(agent_id))
    }

    pub async fn get_seller_by_product(&self, product_id: &str) -> Result<AgentInfo> {
        // This would typically involve a product database lookup
        // For now, we'll return a mock seller
        let agents = self.search_sellers(SearchRequest {
            category: None,
            min_reputation: None,
            payment_methods: None,
        }).await?;

        agents.into_iter()
            .next()
            .ok_or_else(|| NegotiationError::Validation("No sellers found".to_string()))
    }

    async fn search_remote_sellers(&self, request: &SearchRequest) -> Result<Vec<AgentInfo>> {
        let response = self.client
            .post(&format!("{}/search", self.endpoint))
            .json(request)
            .send()
            .await?;

        if response.status().is_success() {
            let search_response: SearchResponse = response.json().await?;
            Ok(search_response.agents)
        } else {
            Err(NegotiationError::Network(response.error_for_status().unwrap_err()))
        }
    }

    pub async fn update_agent_activity(&self, _agent_id: AgentId) -> Result<()> {
        // Update last_active timestamp - would need database integration
        // For now, just log the activity
        tracing::info!("Agent activity updated");
        Ok(())
    }

    pub async fn get_products_by_category(&self, category: &str) -> Result<Vec<AgentInfo>> {
        let sellers = self.search_sellers(SearchRequest {
            category: Some(category.to_string()),
            min_reputation: None,
            payment_methods: None,
        }).await?;

        Ok(sellers)
    }

    pub async fn validate_agent_endpoint(&self, agent_id: AgentId) -> Result<bool> {
        let agent = self.get_agent(agent_id).await?;

        let response = self.client
            .get(&format!("{}/health", agent.endpoint))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

// Discovery server implementation (for standalone discovery service)
#[derive(Clone)]
pub struct DiscoveryServer {
    // database: Database, // Temporarily disabled
}

impl DiscoveryServer {
    pub async fn new(_database_url: &str) -> Result<Self> {
        // let database = Database::new(database_url).await?;
        Ok(Self { /* database */ })
    }

    pub async fn handle_register(&self, request: RegisterRequest) -> Result<AgentInfo> {
        let agent_info = AgentInfo {
            id: uuid::Uuid::new_v4(),
            agent_type: request.agent_type,
            name: request.name,
            endpoint: request.endpoint,
            public_key: request.public_key,
            reputation_score: 100, // New agents start with neutral reputation
            products: vec![],
            payment_methods: request.payment_methods,
            created_at: chrono::Utc::now(),
            last_active: chrono::Utc::now(),
        };

        // self.database.create_agent(&agent_info).await?;
        Ok(agent_info)
    }

    pub async fn handle_search(&self, _request: SearchRequest) -> Result<SearchResponse> {
        // let agents = self.database.get_agents_by_type(AgentType::Seller).await?;

        // Mock implementation
        Ok(SearchResponse {
            agents: vec![],
            total_count: 0,
        })
    }

    pub async fn get_agent_info(&self, _agent_id: AgentId) -> Result<Option<AgentInfo>> {
        // self.database.get_agent(agent_id).await
        Ok(None)
    }

    pub async fn remove_agent(&self, agent_id: AgentId) -> Result<()> {
        // This would require implementing delete operations in the database
        // For now, we'll just log it
        tracing::info!("Agent {} removed from discovery", agent_id);
        Ok(())
    }
}