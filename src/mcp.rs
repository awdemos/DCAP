//! MCP (Model Context Protocol) implementation for DCAP
//!
//! This module provides MCP-compliant tools, resources, and prompts for
//! LLM-to-LLM commerce workflows within the DCAP ecosystem.

use crate::{
    config::AppConfig,
    discovery::{DiscoveryService, RegisterRequest, SearchRequest},
    error::{NegotiationError, Result},
    model::{PaymentMethod, AgentType},
    settlement::SettlementService,
    trust::TrustSystem,
    AgentId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

/// MCP Server for Negotiation Agents
pub struct NegotiationMcpServer {
    config: AppConfig,
    discovery: Arc<RwLock<DiscoveryService>>,
    trust_system: Arc<RwLock<TrustSystem>>,
    settlement: Arc<RwLock<SettlementService>>,
}

impl NegotiationMcpServer {
    /// Create a new MCP server instance
    pub async fn new() -> Result<Self> {
        let config = AppConfig::load("config.toml").unwrap_or_default();

        Ok(Self {
            discovery: Arc::new(RwLock::new(DiscoveryService::new("http://localhost:8000".to_string()))),
            trust_system: Arc::new(RwLock::new(TrustSystem::new()?)),
            settlement: Arc::new(RwLock::new(SettlementService::new(crate::settlement::SettlementConfig {
            stripe_secret_key: None,
            solana_rpc_url: None,
            escrow_service_url: None,
        }).await?)),
            config,
        })
    }

    /// Run the MCP server
    pub async fn run(&self, listener: tokio::net::TcpListener) -> Result<()> {
        // Simple MCP server implementation over TCP
        loop {
            let (socket, addr) = listener.accept().await?;

            let discovery = self.discovery.clone();
            let trust_system = self.trust_system.clone();
            let settlement = self.settlement.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    socket,
                    discovery,
                    trust_system,
                    settlement,
                ).await {
                    eprintln!("Connection error from {}: {}", addr, e);
                }
            });
        }
    }

    async fn handle_connection(
        mut socket: tokio::net::TcpStream,
        discovery: Arc<RwLock<DiscoveryService>>,
        trust_system: Arc<RwLock<TrustSystem>>,
        settlement: Arc<RwLock<SettlementService>>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut buffer = [0; 1024];
        let n = socket.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        // Parse MCP request
        let mcp_request: McpRequest = serde_json::from_str(&request)?;

        // Handle request
        let response = match mcp_request.method.as_str() {
            "tools/call" => {
                Self::handle_tool_call(
                    mcp_request.params,
                    discovery,
                    trust_system,
                    settlement,
                ).await
            },
            "resources/read" => {
                Self::handle_resource_read(
                    mcp_request.params,
                    discovery,
                    trust_system,
                ).await
            },
            "prompts/get" => {
                Self::handle_prompt_get(
                    mcp_request.params,
                ).await
            },
            _ => {
                Err(NegotiationError::InvalidInput("Unknown MCP method".into()))
            }
        };

        // Send response
        let mcp_response = McpResponse {
            id: mcp_request.id,
            result: response.map_err(|e| e.to_string()),
        };

        let response_json = serde_json::to_string(&mcp_response)?;
        socket.write_all(response_json.as_bytes()).await?;

        Ok(())
    }

    async fn handle_tool_call(
        params: serde_json::Value,
        discovery: Arc<RwLock<DiscoveryService>>,
        trust_system: Arc<RwLock<TrustSystem>>,
        settlement: Arc<RwLock<SettlementService>>,
    ) -> Result<serde_json::Value> {
        let tool_call: ToolCall = serde_json::from_value(params)?;

        match tool_call.name.as_str() {
            "register_agent" => {
                let request: RegisterRequest = serde_json::from_value(tool_call.arguments)?;
                let mut discovery = discovery.write().await;
                // Mock agent info creation
                let agent_info = crate::model::AgentInfo {
                    id: AgentId::new_v4(),
                    agent_type: request.agent_type,
                    name: request.name,
                    endpoint: request.endpoint,
                    public_key: request.public_key,
                    reputation_score: 50,
                    products: vec![],
                    payment_methods: request.payment_methods,
                    created_at: chrono::Utc::now(),
                    last_active: chrono::Utc::now(),
                };
                let result = discovery.register_agent(agent_info).await?;
                Ok(serde_json::to_value(result)?)
            },
            "search_agents" => {
                let request: SearchRequest = serde_json::from_value(tool_call.arguments)?;
                let mut discovery = discovery.write().await;
                let result = discovery.search_sellers(request).await?;
                Ok(serde_json::to_value(result)?)
            },
            "get_reputation" => {
                let rep_req: ReputationRequest = serde_json::from_value(tool_call.arguments)?;
                let trust_system = trust_system.read().await;
                let score = trust_system.get_reputation(rep_req.agent_id).await?;
                Ok(serde_json::to_value(score)?)
            },
            "update_reputation" => {
                let update_req: ReputationUpdateRequest = serde_json::from_value(tool_call.arguments)?;
                let mut trust_system = trust_system.write().await;
                trust_system.update_reputation(update_req.agent_id, update_req.score_change).await?;
                Ok(serde_json::to_value("Reputation updated")?)
            },
            _ => {
                Err(NegotiationError::InvalidInput(format!("Unknown tool: {}", tool_call.name)))
            }
        }
    }

    async fn handle_resource_read(
        params: serde_json::Value,
        discovery: Arc<RwLock<DiscoveryService>>,
        trust_system: Arc<RwLock<TrustSystem>>,
    ) -> Result<serde_json::Value> {
        let resource_req: ResourceRequest = serde_json::from_value(params)?;

        match resource_req.uri.as_str() {
            "agent://reputations" => {
                let trust_system = trust_system.read().await;
                let reputations = trust_system.get_all_reputations().await?;
                Ok(serde_json::to_value(reputations)?)
            },
            "product://catalog" => {
                // Mock product catalog for now
                let mock_catalog = vec![
                    crate::model::Product {
                        id: "laptop-001".into(),
                        name: "Gaming Laptop Pro".into(),
                        description: "High-performance gaming laptop with RTX 4080".into(),
                        category: "Electronics".into(),
                        base_price: 2499.99,
                        currency: "USD".into(),
                        stock_quantity: 15,
                        metadata: std::collections::HashMap::new(),
                    },
                    crate::model::Product {
                        id: "keyboard-002".into(),
                        name: "Mechanical Keyboard RGB".into(),
                        description: "Premium mechanical keyboard with RGB lighting".into(),
                        category: "Electronics".into(),
                        base_price: 129.99,
                        currency: "USD".into(),
                        stock_quantity: 50,
                        metadata: std::collections::HashMap::new(),
                    },
                    crate::model::Product {
                        id: "monitor-003".into(),
                        name: "4K Monitor 27\"".into(),
                        description: "Ultra HD 27-inch monitor with HDR support".into(),
                        category: "Electronics".into(),
                        base_price: 399.99,
                        currency: "USD".into(),
                        stock_quantity: 25,
                        metadata: std::collections::HashMap::new(),
                    },
                ];
                Ok(serde_json::to_value(mock_catalog)?)
            },
            "agent://active" => {
                // Mock active agents
                let mock_agents = vec![
                    crate::model::AgentInfo {
                        id: AgentId::new_v4(),
                        agent_type: crate::model::AgentType::Seller,
                        name: "TechStore Pro".into(),
                        endpoint: "http://localhost:8001".into(),
                        public_key: "mock_public_key_1".into(),
                        reputation_score: 85,
                        products: vec![],
                        payment_methods: vec![crate::model::PaymentMethod::Stripe],
                        created_at: chrono::Utc::now(),
                        last_active: chrono::Utc::now(),
                    },
                    crate::model::AgentInfo {
                        id: AgentId::new_v4(),
                        agent_type: crate::model::AgentType::Seller,
                        name: "GadgetHub".into(),
                        endpoint: "http://localhost:8002".into(),
                        public_key: "mock_public_key_2".into(),
                        reputation_score: 72,
                        products: vec![],
                        payment_methods: vec![crate::model::PaymentMethod::Stripe, crate::model::PaymentMethod::Escrow],
                        created_at: chrono::Utc::now(),
                        last_active: chrono::Utc::now(),
                    },
                ];
                Ok(serde_json::to_value(mock_agents)?)
            },
            "negotiation://history" => {
                // Mock negotiation history
                let mock_history = serde_json::json!({
                    "negotiations": [
                        {
                            "id": "neg-001",
                            "product_id": "laptop-001",
                            "buyer_id": "buyer-123",
                            "seller_id": "seller-456",
                            "initial_price": 2499.99,
                            "final_price": 2299.99,
                            "status": "completed",
                            "timestamp": "2024-01-15T10:30:00Z"
                        },
                        {
                            "id": "neg-002",
                            "product_id": "keyboard-002",
                            "buyer_id": "buyer-789",
                            "seller_id": "seller-456",
                            "initial_price": 129.99,
                            "final_price": 119.99,
                            "status": "completed",
                            "timestamp": "2024-01-15T14:20:00Z"
                        }
                    ],
                    "total_count": 2
                });
                Ok(mock_history)
            },
            "market://analytics" => {
                // Mock market analytics
                let mock_analytics = serde_json::json!({
                    "categories": {
                        "Electronics": {
                            "total_volume": 1250000.00,
                            "average_price": 456.78,
                            "trend": "increasing",
                            "volatility": 0.12
                        },
                        "Accessories": {
                            "total_volume": 345000.00,
                            "average_price": 89.99,
                            "trend": "stable",
                            "volatility": 0.08
                        }
                    },
                    "top_products": [
                        {"id": "laptop-001", "volume": 450000.00},
                        {"id": "keyboard-002", "volume": 125000.00},
                        {"id": "monitor-003", "volume": 98000.00}
                    ]
                });
                Ok(mock_analytics)
            },
            _ => {
                Ok(serde_json::json!({"error": "Resource not found", "uri": resource_req.uri}))
            }
        }
    }

    async fn handle_prompt_get(params: serde_json::Value) -> Result<serde_json::Value> {
        let prompt_req: PromptRequest = serde_json::from_value(params)?;

        match prompt_req.name.as_str() {
            "negotiation_strategy" => {
                let prompt = NegotiationPrompt::strategy();
                Ok(serde_json::to_value(prompt)?)
            },
            "price_optimization" => {
                let prompt = NegotiationPrompt::price_optimization();
                Ok(serde_json::to_value(prompt)?)
            },
            "market_analysis" => {
                let prompt = NegotiationPrompt::market_analysis();
                Ok(serde_json::to_value(prompt)?)
            },
            "counter_offer" => {
                let prompt = NegotiationPrompt::counter_offer();
                Ok(serde_json::to_value(prompt)?)
            },
            "agent_communication" => {
                let prompt = NegotiationPrompt::agent_communication();
                Ok(serde_json::to_value(prompt)?)
            },
            "trust_assessment" => {
                let prompt = NegotiationPrompt::trust_assessment();
                Ok(serde_json::to_value(prompt)?)
            },
            _ => {
                Err(NegotiationError::InvalidInput(format!("Unknown prompt: {}", prompt_req.name)))
            }
        }
    }
}

// MCP Request/Response types
#[derive(Debug, Serialize, Deserialize)]
struct McpRequest {
    id: String,
    method: String,
    params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpResponse {
    id: String,
    result: std::result::Result<serde_json::Value, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResourceRequest {
    uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReputationRequest {
    agent_id: AgentId,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReputationUpdateRequest {
    agent_id: AgentId,
    score_change: i32,
}

// MCP Prompts
#[derive(Debug, Serialize, Deserialize)]
struct NegotiationPrompt {
    name: String,
    description: String,
    template: String,
    variables: Vec<PromptVariable>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptVariable {
    name: String,
    description: String,
    required: bool,
}

impl NegotiationPrompt {
    fn strategy() -> Self {
        Self {
            name: "negotiation_strategy".into(),
            description: "Generate a negotiation strategy for a given product and market conditions".into(),
            template: r#"
You are a negotiation agent for {{product_name}} in the {{category}} category.

Market Context:
- Current market price: ${{market_price}}
- Your reservation price: ${{reservation_price}}
- Buyer's maximum price: ${{buyer_max_price}}
- Your reputation score: {{reputation_score}}/100

Generate a negotiation strategy that:
1. Considers your reputation and desired profit margin
2. Accounts for market conditions and competitor pricing
3. Includes specific opening offers and counter-offer strategies
4. Considers the urgency of the transaction
5. Maximizes value while maintaining good buyer relationships

Strategy:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "product_name".into(),
                    description: "Name of the product being negotiated".into(),
                    required: true,
                },
                PromptVariable {
                    name: "category".into(),
                    description: "Product category".into(),
                    required: true,
                },
                PromptVariable {
                    name: "market_price".into(),
                    description: "Current market price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "reservation_price".into(),
                    description: "Your minimum acceptable price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "buyer_max_price".into(),
                    description: "Buyer's maximum willing price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "reputation_score".into(),
                    description: "Your reputation score (0-100)".into(),
                    required: true,
                },
            ],
        }
    }

    fn price_optimization() -> Self {
        Self {
            name: "price_optimization".into(),
            description: "Optimize pricing strategy based on market data and competitor analysis".into(),
            template: r#"
You are a pricing optimization agent for {{product_name}}.

Available Data:
- Historical sales data: {{sales_data}}
- Competitor prices: {{competitor_prices}}
- Market demand: {{demand_level}}
- Inventory levels: {{inventory_level}}
- Seasonal trends: {{seasonal_trends}}

Generate an optimized pricing strategy that:
1. Maximizes revenue based on current market conditions
2. Considers inventory turnover goals
3. Accounts for competitive positioning
4. Includes dynamic pricing recommendations
5. Provides confidence intervals for price points

Optimization Strategy:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "product_name".into(),
                    description: "Product name".into(),
                    required: true,
                },
                PromptVariable {
                    name: "sales_data".into(),
                    description: "Historical sales data".into(),
                    required: true,
                },
                PromptVariable {
                    name: "competitor_prices".into(),
                    description: "Current competitor prices".into(),
                    required: true,
                },
                PromptVariable {
                    name: "demand_level".into(),
                    description: "Current market demand level".into(),
                    required: true,
                },
                PromptVariable {
                    name: "inventory_level".into(),
                    description: "Current inventory level".into(),
                    required: true,
                },
                PromptVariable {
                    name: "seasonal_trends".into(),
                    description: "Seasonal demand trends".into(),
                    required: true,
                },
            ],
        }
    }

    fn market_analysis() -> Self {
        Self {
            name: "market_analysis".into(),
            description: "Analyze market conditions and provide insights for negotiation decisions".into(),
            template: r#"
You are a market analysis agent providing insights for {{product_category}} negotiations.

Market Data:
- Average transaction volume: {{avg_volume}}
- Price volatility: {{price_volatility}}
- Market sentiment: {{market_sentiment}}
- Key trends: {{key_trends}}
- Regulatory environment: {{regulatory_env}}

Provide analysis covering:
1. Current market conditions and their impact on negotiations
2. Risk factors that could affect transaction outcomes
3. Opportunities for favorable terms
4. Recommended negotiation timing and approach
5. Market-specific considerations for this category

Market Analysis:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "product_category".into(),
                    description: "Product category being analyzed".into(),
                    required: true,
                },
                PromptVariable {
                    name: "avg_volume".into(),
                    description: "Average transaction volume".into(),
                    required: true,
                },
                PromptVariable {
                    name: "price_volatility".into(),
                    description: "Current price volatility".into(),
                    required: true,
                },
                PromptVariable {
                    name: "market_sentiment".into(),
                    description: "Current market sentiment".into(),
                    required: true,
                },
                PromptVariable {
                    name: "key_trends".into(),
                    description: "Key market trends".into(),
                    required: true,
                },
                PromptVariable {
                    name: "regulatory_env".into(),
                    description: "Regulatory environment".into(),
                    required: true,
                },
            ],
        }
    }

    fn counter_offer() -> Self {
        Self {
            name: "counter_offer".into(),
            description: "Generate a strategic counter-offer response for an ongoing negotiation".into(),
            template: r#"
You are a negotiation agent responding to an offer for {{product_name}}.

Current Negotiation State:
- Original asking price: ${{original_price}}
- Buyer's offer: ${{buyer_offer}}
- Your minimum acceptable price: ${{min_price}}
- Market average: ${{market_price}}
- Urgency level: {{urgency_level}}
- Buyer's reputation: {{buyer_reputation}}/100

Generate a counter-offer that:
1. Is reasonable but favorable to your position
2. Includes justification for the price
3. Maintains good relationship with the buyer
4. Considers market conditions and urgency
5. May include value-added terms (free shipping, warranty, etc.)

Counter-Offer Response:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "product_name".into(),
                    description: "Name of the product".into(),
                    required: true,
                },
                PromptVariable {
                    name: "original_price".into(),
                    description: "Original asking price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "buyer_offer".into(),
                    description: "Buyer's current offer".into(),
                    required: true,
                },
                PromptVariable {
                    name: "min_price".into(),
                    description: "Your minimum acceptable price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "market_price".into(),
                    description: "Current market price".into(),
                    required: true,
                },
                PromptVariable {
                    name: "urgency_level".into(),
                    description: "How urgent the sale is (low/medium/high)".into(),
                    required: true,
                },
                PromptVariable {
                    name: "buyer_reputation".into(),
                    description: "Buyer's reputation score (0-100)".into(),
                    required: true,
                },
            ],
        }
    }

    fn agent_communication() -> Self {
        Self {
            name: "agent_communication".into(),
            description: "Generate professional communication messages between negotiation agents".into(),
            template: r#"
You are {{agent_role}} negotiating {{product_name}} with {{counterparty_role}}.

Communication Context:
- Previous messages: {{conversation_history}}
- Current negotiation stage: {{negotiation_stage}}
- Your position strength: {{position_strength}}
- Desired outcome: {{desired_outcome}}
- Communication tone: {{tone}}

Generate a professional communication message that:
1. Clearly states your position or response
2. Maintains appropriate business etiquette
3. Builds rapport with the counterparty
4. Moves the negotiation forward constructively
5. Includes specific details and next steps

{{agent_role}} Message:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "agent_role".into(),
                    description: "Your role (buyer/seller)".into(),
                    required: true,
                },
                PromptVariable {
                    name: "counterparty_role".into(),
                    description: "Other party's role (buyer/seller)".into(),
                    required: true,
                },
                PromptVariable {
                    name: "product_name".into(),
                    description: "Product being negotiated".into(),
                    required: true,
                },
                PromptVariable {
                    name: "conversation_history".into(),
                    description: "Previous messages in the negotiation".into(),
                    required: true,
                },
                PromptVariable {
                    name: "negotiation_stage".into(),
                    description: "Current stage of negotiation".into(),
                    required: true,
                },
                PromptVariable {
                    name: "position_strength".into(),
                    description: "Your negotiating position strength".into(),
                    required: true,
                },
                PromptVariable {
                    name: "desired_outcome".into(),
                    description: "What you want to achieve".into(),
                    required: true,
                },
                PromptVariable {
                    name: "tone".into(),
                    description: "Desired communication tone".into(),
                    required: true,
                },
            ],
        }
    }

    fn trust_assessment() -> Self {
        Self {
            name: "trust_assessment".into(),
            description: "Assess trustworthiness of a counterparty agent for negotiation decisions".into(),
            template: r#"
You are conducting a trust assessment for a potential negotiation with {{counterparty_name}}.

Counterparty Profile:
- Agent ID: {{agent_id}}
- Reputation score: {{reputation_score}}/100
- Successful transactions: {{successful_transactions}}
- Failed transactions: {{failed_transactions}}
- Account age: {{account_age}}
- Average response time: {{response_time}}
- Verification status: {{verification_status}}
- Market presence: {{market_presence}}

Assessment Factors:
1. **Reputation Analysis**: Evaluate the reputation score in context
2. **Transaction History**: Analyze success/failure patterns
3. **Responsiveness**: Consider communication timeliness
4. **Market Standing**: Assess their position in the market
5. **Risk Factors**: Identify potential concerns

Generate a comprehensive trust assessment that includes:
- Overall trustworthiness rating (low/medium/high)
- Key strengths that build confidence
- Potential risks or concerns
- Recommended negotiation approach
- Trust-building strategies
- Risk mitigation measures

Trust Assessment Report:
"#.into(),
            variables: vec![
                PromptVariable {
                    name: "counterparty_name".into(),
                    description: "Name of the counterparty".into(),
                    required: true,
                },
                PromptVariable {
                    name: "agent_id".into(),
                    description: "Unique identifier of the agent".into(),
                    required: true,
                },
                PromptVariable {
                    name: "reputation_score".into(),
                    description: "Reputation score (0-100)".into(),
                    required: true,
                },
                PromptVariable {
                    name: "successful_transactions".into(),
                    description: "Number of successful transactions".into(),
                    required: true,
                },
                PromptVariable {
                    name: "failed_transactions".into(),
                    description: "Number of failed transactions".into(),
                    required: true,
                },
                PromptVariable {
                    name: "account_age".into(),
                    description: "How long the account has existed".into(),
                    required: true,
                },
                PromptVariable {
                    name: "response_time".into(),
                    description: "Average response time".into(),
                    required: true,
                },
                PromptVariable {
                    name: "verification_status".into(),
                    description: "Verification status of the agent".into(),
                    required: true,
                },
                PromptVariable {
                    name: "market_presence".into(),
                    description: "Agent's presence in the market".into(),
                    required: true,
                },
            ],
        }
    }
}