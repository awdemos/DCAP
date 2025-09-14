use crate::{AgentId, NegotiationError, Result, TransactionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub agent_type: AgentType,
    pub name: String,
    pub endpoint: String,
    pub public_key: String,
    pub reputation_score: u32,
    pub products: Vec<Product>,
    pub payment_methods: Vec<PaymentMethod>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Buyer,
    Seller,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub base_price: f64,
    pub currency: String,
    pub stock_quantity: u32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RFQ {
    pub id: TransactionId,
    pub buyer_id: AgentId,
    pub product_id: String,
    pub quantity: u32,
    pub max_price: f64,
    pub currency: String,
    pub delivery_location: Option<String>,
    pub deadline: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: TransactionId,
    pub rfq_id: TransactionId,
    pub seller_id: AgentId,
    pub price: f64,
    pub currency: String,
    pub available_quantity: u32,
    pub delivery_estimate: Option<String>,
    pub ttl_seconds: u32,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Negotiation {
    pub id: TransactionId,
    pub rfq_id: TransactionId,
    pub quote_id: Option<TransactionId>,
    pub buyer_id: AgentId,
    pub seller_id: AgentId,
    pub product_id: String,
    pub quantity: u32,
    pub opening_bid: f64,
    pub close_price: Option<f64>,
    pub delta: Option<f64>,
    pub status: NegotiationStatus,
    pub messages: Vec<NegotiationMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NegotiationStatus {
    Pending,
    Quoted,
    Negotiating,
    Accepted,
    Rejected,
    Expired,
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationMessage {
    pub id: Uuid,
    pub negotiation_id: TransactionId,
    pub sender_id: AgentId,
    pub content: String,
    pub message_type: MessageType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    RFQ,
    Quote,
    CounterOffer,
    Accept,
    Reject,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationRecord {
    pub buyer_id: AgentId,
    pub seller_id: AgentId,
    pub product_hash: String,
    pub opening_bid: f64,
    pub close_price: f64,
    pub delta: f64,
    pub timestamp: DateTime<Utc>,
    pub duration_seconds: u64,
    pub message_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PaymentMethod {
    Stripe,
    Solana,
    Escrow,
}

impl RFQ {
    pub fn new(
        buyer_id: AgentId,
        product_id: String,
        quantity: u32,
        max_price: f64,
        currency: String,
        deadline: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            buyer_id,
            product_id,
            quantity,
            max_price,
            currency,
            delivery_location: None,
            deadline,
            metadata: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.quantity == 0 {
            return Err(NegotiationError::Validation("Quantity must be greater than 0".to_string()));
        }
        if self.max_price <= 0.0 {
            return Err(NegotiationError::Validation("Max price must be greater than 0".to_string()));
        }
        if self.deadline <= Utc::now() {
            return Err(NegotiationError::Validation("Deadline must be in the future".to_string()));
        }
        Ok(())
    }
}

impl Quote {
    pub fn new(
        rfq_id: TransactionId,
        seller_id: AgentId,
        price: f64,
        currency: String,
        available_quantity: u32,
        ttl_seconds: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            rfq_id,
            seller_id,
            price,
            currency,
            available_quantity,
            delivery_estimate: None,
            ttl_seconds,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.created_at + chrono::Duration::seconds(self.ttl_seconds as i64)
    }

    pub fn validate(&self) -> Result<()> {
        if self.price <= 0.0 {
            return Err(NegotiationError::Validation("Price must be greater than 0".to_string()));
        }
        if self.available_quantity == 0 {
            return Err(NegotiationError::Validation("Available quantity must be greater than 0".to_string()));
        }
        if self.ttl_seconds == 0 {
            return Err(NegotiationError::Validation("TTL must be greater than 0".to_string()));
        }
        Ok(())
    }
}

impl Negotiation {
    pub fn new(rfq: RFQ, seller_id: AgentId) -> Self {
        Self {
            id: Uuid::new_v4(),
            rfq_id: rfq.id,
            quote_id: None,
            buyer_id: rfq.buyer_id,
            seller_id,
            product_id: rfq.product_id,
            quantity: rfq.quantity,
            opening_bid: rfq.max_price,
            close_price: None,
            delta: None,
            status: NegotiationStatus::Pending,
            messages: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn add_quote(&mut self, quote: &Quote) -> Result<()> {
        if self.quote_id.is_some() {
            return Err(NegotiationError::Negotiation("Quote already exists for this negotiation".to_string()));
        }
        self.quote_id = Some(quote.id);
        self.status = NegotiationStatus::Quoted;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn accept(&mut self, final_price: f64) -> Result<()> {
        if self.status != NegotiationStatus::Quoted && self.status != NegotiationStatus::Negotiating {
            return Err(NegotiationError::Negotiation("Cannot accept negotiation in current state".to_string()));
        }
        self.close_price = Some(final_price);
        self.delta = Some(final_price - self.opening_bid);
        self.status = NegotiationStatus::Accepted;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn reject(&mut self) -> Result<()> {
        if self.status != NegotiationStatus::Quoted && self.status != NegotiationStatus::Negotiating {
            return Err(NegotiationError::Negotiation("Cannot reject negotiation in current state".to_string()));
        }
        self.status = NegotiationStatus::Rejected;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn settle(&mut self) -> Result<()> {
        if self.status != NegotiationStatus::Accepted {
            return Err(NegotiationError::Negotiation("Cannot settle unaccepted negotiation".to_string()));
        }
        self.status = NegotiationStatus::Settled;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn to_record(&self) -> Option<NegotiationRecord> {
        if let (Some(close_price), Some(delta)) = (self.close_price, self.delta) {
            Some(NegotiationRecord {
                buyer_id: self.buyer_id,
                seller_id: self.seller_id,
                product_hash: self.product_id.clone(),
                opening_bid: self.opening_bid,
                close_price,
                delta,
                timestamp: self.created_at,
                duration_seconds: (self.updated_at - self.created_at).num_seconds() as u64,
                message_count: self.messages.len() as u32,
            })
        } else {
            None
        }
    }
}