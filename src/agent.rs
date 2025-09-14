use crate::{
    discovery::{DiscoveryService, SearchRequest},
    error::{NegotiationError, Result},
    model::*,
    settlement::SettlementService,
    trust::TrustSystem,
    AgentId, TransactionId,
};
use chrono::{Duration, Utc, Timelike};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use base64::{engine::general_purpose, Engine};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyerAgentConfig {
    pub agent_id: AgentId,
    pub name: String,
    pub endpoint: String,
    pub max_concurrent_negotiations: u32,
    pub default_ttl_hours: u32,
    pub llm_config: LLMConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SellerAgentConfig {
    pub agent_id: AgentId,
    pub name: String,
    pub endpoint: String,
    pub products: Vec<Product>,
    pub payment_methods: Vec<PaymentMethod>,
    pub llm_config: LLMConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMConfig {
    pub model: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub temperature: f64,
}

pub struct BuyerAgent {
    config: BuyerAgentConfig,
    client: Client,
    discovery: DiscoveryService,
    trust: TrustSystem,
    settlement: SettlementService,
    active_negotiations: HashMap<TransactionId, Negotiation>,
}

impl BuyerAgent {
    pub async fn new(
        config: BuyerAgentConfig,
        discovery: DiscoveryService,
        trust: TrustSystem,
        settlement: SettlementService,
    ) -> Result<Self> {
        let client = Client::new();
        Ok(Self {
            config,
            client,
            discovery,
            trust,
            settlement,
            active_negotiations: HashMap::new(),
        })
    }

    pub async fn browse_products(&self, category: Option<String>) -> Result<Vec<Product>> {
        let sellers = self.discovery.search_sellers(SearchRequest {
            category,
            min_reputation: None,
            payment_methods: None,
        }).await?;

        let mut all_products = Vec::new();
        for seller in sellers {
            let response = self.client
                .get(&format!("{}/products", seller.endpoint))
                .send()
                .await?;

            if response.status().is_success() {
                let products: Vec<Product> = response.json().await?;
                all_products.extend(products);
            }
        }

        Ok(all_products)
    }

    pub async fn request_quote(&mut self, product_id: String, quantity: u32, max_price: f64) -> Result<TransactionId> {
        let product = self.find_product(&product_id).await?;

        if quantity > product.stock_quantity {
            return Err(NegotiationError::Validation("Insufficient stock quantity".to_string()));
        }

        let deadline = Utc::now() + Duration::hours(self.config.default_ttl_hours as i64);
        let rfq = RFQ::new(
            self.config.agent_id,
            product_id.clone(),
            quantity,
            max_price,
            product.currency.clone(),
            deadline,
        );

        rfq.validate()?;

        let seller = self.discovery.get_seller_by_product(&product_id).await?;
        let negotiation = Negotiation::new(rfq.clone(), seller.id);

        // self.database.create_negotiation(&negotiation).await?;
        self.active_negotiations.insert(negotiation.id, negotiation.clone());

        let response = self.client
            .post(&format!("{}/quote", seller.endpoint))
            .json(&rfq)
            .send()
            .await?;

        if response.status().is_success() {
            let quote: Quote = response.json().await?;
            let negotiation = self.active_negotiations.get_mut(&negotiation.id).unwrap();
            negotiation.add_quote(&quote)?;
            // self.database.update_negotiation(negotiation).await?;
            Ok(negotiation.id)
        } else {
            Err(NegotiationError::Network(response.error_for_status().unwrap_err()))
        }
    }

    pub async fn negotiate(&mut self, negotiation_id: TransactionId, counter_offer: f64) -> Result<()> {
        let negotiation = self.active_negotiations.get_mut(&negotiation_id)
            .ok_or(NegotiationError::Validation("Negotiation not found".to_string()))?;

        if counter_offer <= negotiation.opening_bid {
            return Err(NegotiationError::Validation("Counter offer must be less than opening bid".to_string()));
        }

        let seller = self.discovery.get_agent(negotiation.seller_id).await?;
        let response = self.client
            .post(&format!("{}/negotiate/{}", seller.endpoint, negotiation_id))
            .json(&serde_json::json!({
                "counter_offer": counter_offer
            }))
            .send()
            .await?;

        if response.status().is_success() {
            let quote: Quote = response.json().await?;
            negotiation.add_quote(&quote)?;
            // self.database.update_negotiation(negotiation).await?;
            Ok(())
        } else {
            Err(NegotiationError::Network(response.error_for_status().unwrap_err()))
        }
    }

    pub async fn accept_quote(&mut self, negotiation_id: TransactionId) -> Result<()> {
        let quote = self.get_quote_for_negotiation(negotiation_id).await?;
        let negotiation = self.active_negotiations.get_mut(&negotiation_id)
            .ok_or(NegotiationError::Validation("Negotiation not found".to_string()))?;

        if negotiation.quote_id.is_none() {
            return Err(NegotiationError::Negotiation("No quote available".to_string()));
        }

        negotiation.accept(quote.price)?;
        // self.database.update_negotiation(negotiation).await?;

        let payment_result = self.settlement.create_payment(
            negotiation.buyer_id,
            negotiation.seller_id,
            quote.price,
            quote.currency.clone(),
        ).await?;

        if payment_result.success {
            negotiation.settle()?;
            // self.database.update_negotiation(negotiation).await?;

            if let Some(_record) = negotiation.to_record() {
                // self.database.add_negotiation_record(&record).await?;
            }

            self.trust.update_reputation(negotiation.seller_id, 5).await?;
            self.trust.update_reputation(negotiation.buyer_id, 3).await?;
        }

        Ok(())
    }

    pub async fn reject_quote(&mut self, negotiation_id: TransactionId) -> Result<()> {
        let negotiation = self.active_negotiations.get_mut(&negotiation_id)
            .ok_or(NegotiationError::Validation("Negotiation not found".to_string()))?;

        negotiation.reject()?;
        // self.database.update_negotiation(negotiation).await?;

        self.trust.update_reputation(negotiation.seller_id, -2).await?;
        Ok(())
    }

    async fn find_product(&self, product_id: &str) -> Result<Product> {
        let response = self.client
            .get(&format!("{}/discovery/products/{}", self.discovery.endpoint(), product_id))
            .send()
            .await?;

        if response.status().is_success() {
            let product: Product = response.json().await?;
            Ok(product)
        } else {
            Err(NegotiationError::ProductNotFound(product_id.to_string()))
        }
    }

    async fn get_quote_for_negotiation(&self, negotiation_id: TransactionId) -> Result<Quote> {
        // For now, we'll look for the negotiation in active negotiations
        let negotiation = self.active_negotiations.get(&negotiation_id)
            .ok_or(NegotiationError::Validation("Negotiation not found".to_string()))?;

        let seller = self.discovery.get_agent(negotiation.seller_id).await?;
        let response = self.client
            .get(&format!("{}/quote/{}", seller.endpoint, negotiation.rfq_id))
            .send()
            .await?;

        if response.status().is_success() {
            let quote: Quote = response.json().await?;
            Ok(quote)
        } else {
            Err(NegotiationError::Negotiation("Quote not found".to_string()))
        }
    }

    pub fn get_active_negotiations(&self) -> Vec<&Negotiation> {
        self.active_negotiations.values().collect()
    }
}

pub struct SellerAgent {
    config: SellerAgentConfig,
    discovery: DiscoveryService,
    trust: TrustSystem,
}

impl SellerAgent {
    pub async fn new(
        config: SellerAgentConfig,
        discovery: DiscoveryService,
        trust: TrustSystem,
    ) -> Result<Self> {
        Ok(Self {
            config,
            discovery,
            trust,
        })
    }

    pub async fn register(&self) -> Result<()> {
        let agent_info = AgentInfo {
            id: self.config.agent_id,
            agent_type: AgentType::Seller,
            name: self.config.name.clone(),
            endpoint: self.config.endpoint.clone(),
            public_key: generate_public_key().await?,
            reputation_score: 100,
            products: self.config.products.clone(),
            payment_methods: self.config.payment_methods.clone(),
            created_at: Utc::now(),
            last_active: Utc::now(),
        };

        self.discovery.register_agent(agent_info).await?;
        Ok(())
    }

    pub async fn handle_rfq(&mut self, rfq: RFQ) -> Result<Quote> {
        let product_id = rfq.product_id.clone();
        let product = self.config.products.iter()
            .find(|p| p.id == product_id)
            .ok_or(NegotiationError::ProductNotFound(product_id))?;

        if rfq.quantity > product.stock_quantity {
            return Err(NegotiationError::Validation("Insufficient stock".to_string()));
        }

        let buyer_reputation = self.trust.get_reputation(rfq.buyer_id).await?;
        if buyer_reputation < 50 {
            return Err(NegotiationError::InsufficientReputation(buyer_reputation));
        }

        let base_price = product.base_price * rfq.quantity as f64;
        let dynamic_pricing_factor = self.calculate_dynamic_pricing(&rfq, buyer_reputation).await?;
        let final_price = base_price * dynamic_pricing_factor;

        let quote = Quote::new(
            rfq.id,
            self.config.agent_id,
            final_price,
            product.currency.clone(),
            rfq.quantity,
            3600, // 1 hour TTL
        );

        Ok(quote)
    }

    pub async fn handle_negotiation(&self, negotiation_id: TransactionId, counter_offer: f64) -> Result<Quote> {
        // For now, this is a mock implementation since database is not implemented
        // let negotiation = self.database.get_negotiation(negotiation_id).await?
        //     .ok_or(NegotiationError::Validation("Negotiation not found".to_string()))?;

        // Mock negotiation data - in real implementation this would come from database
        let buyer_id = uuid::Uuid::new_v4();
        let opening_bid = 100.0; // Mock opening bid

        if buyer_id == self.config.agent_id {
            return Err(NegotiationError::Auth("Unauthorized negotiation".to_string()));
        }

        let min_acceptable_price = opening_bid * 0.8; // 20% minimum discount
        if counter_offer < min_acceptable_price {
            return Err(NegotiationError::Negotiation("Counter offer too low".to_string()));
        }

        let buyer_reputation = self.trust.get_reputation(buyer_id).await?;
        let acceptance_threshold = match buyer_reputation {
            score if score >= 80 => 0.95, // High trust buyers get better terms
            score if score >= 60 => 0.90,
            _ => 0.85,
        };

        let adjusted_price = counter_offer * acceptance_threshold;
        let quote = Quote::new(
            negotiation_id, // Using negotiation_id as rfq_id for mock
            self.config.agent_id,
            adjusted_price,
            "USD".to_string(), // Should come from product
            1, // Mock quantity
            1800, // 30 minutes TTL for counter offers
        );

        Ok(quote)
    }

    async fn calculate_dynamic_pricing(&self, rfq: &RFQ, buyer_reputation: u32) -> Result<f64> {
        let mut factor = 1.0;

        // Volume discount
        if rfq.quantity > 10 {
            factor *= 0.95;
        }

        // Reputation bonus
        if buyer_reputation >= 80 {
            factor *= 0.98;
        }

        // Time-based pricing
        let hour = Utc::now().hour();
        if hour >= 9 && hour <= 17 { // Business hours
            factor *= 1.02;
        }

        // Demand-based pricing (placeholder - would integrate with market data)
        factor *= 1.01;

        Ok(factor)
    }
}

async fn generate_public_key() -> Result<String> {
    // For now, return a mock public key
    // In production, this would generate a real Ed25519 keypair
    Ok("mock_public_key_base64_encoded".to_string())
}