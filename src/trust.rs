use crate::{
    error::{NegotiationError, Result},
    AgentId,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReputationScore {
    pub agent_id: AgentId,
    pub score: u32,
    pub successful_transactions: u32,
    pub failed_transactions: u32,
    pub total_negotiations: u32,
    pub average_response_time_ms: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Untrusted,    // 0-49
    Neutral,      // 50-74
    Trusted,      // 75-89
    HighlyTrusted, // 90-100
}

impl From<u32> for TrustLevel {
    fn from(score: u32) -> Self {
        match score {
            0..=49 => TrustLevel::Untrusted,
            50..=74 => TrustLevel::Neutral,
            75..=89 => TrustLevel::Trusted,
            _ => TrustLevel::HighlyTrusted,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JWTClaims {
    pub sub: String, // agent_id
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub reputation_score: u32,
    pub trust_level: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustActivity {
    pub id: uuid::Uuid,
    pub agent_id: AgentId,
    pub activity_type: TrustActivityType,
    pub score_change: i32,
    pub reason: String,
    pub related_agent_id: Option<AgentId>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustActivityType {
    SuccessfulTransaction,
    FailedTransaction,
    QuoteExpired,
    NegotiationRejected,
    ReputationReport,
    SystemAdjustment,
}

pub struct TrustSystem {
    jwt_secret: String,
    reputation_cache: HashMap<AgentId, ReputationScore>,
    cache_ttl: Duration,
}

impl TrustSystem {
    pub fn new() -> Result<Self> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key-here".to_string());

        Ok(Self {
            jwt_secret,
            reputation_cache: HashMap::new(),
            cache_ttl: Duration::minutes(30),
        })
    }

    pub async fn get_reputation(&self, agent_id: AgentId) -> Result<u32> {
        // Check cache first
        if let Some(cached) = self.reputation_cache.get(&agent_id) {
            if Utc::now() - cached.last_updated < self.cache_ttl {
                return Ok(cached.score);
            }
        }

        // New agents start with 0 reputation
        Ok(0)
    }

    pub async fn update_reputation(&mut self, agent_id: AgentId, score_change: i32) -> Result<()> {
        let current_score = self.get_reputation(agent_id).await?;
        let new_score = (current_score as i32 + score_change).max(0).min(100) as u32;

        // Update cache
        let reputation_score = ReputationScore {
            agent_id,
            score: new_score,
            successful_transactions: 0,
            failed_transactions: 0,
            total_negotiations: 0,
            average_response_time_ms: 0,
            last_updated: Utc::now(),
            trust_level: TrustLevel::from(new_score),
        };
        self.reputation_cache.insert(agent_id, reputation_score);

        // Log the activity
        self.log_trust_activity(TrustActivity {
            id: uuid::Uuid::new_v4(),
            agent_id,
            activity_type: TrustActivityType::SystemAdjustment,
            score_change,
            reason: format!("Reputation adjusted by {}", score_change),
            related_agent_id: None,
            timestamp: Utc::now(),
        }).await?;

        Ok(())
    }

    pub async fn record_successful_transaction(&mut self, buyer_id: AgentId, seller_id: AgentId) -> Result<()> {
        // Both parties get reputation boost for successful transactions
        self.update_reputation(buyer_id, 5).await?;
        self.update_reputation(seller_id, 5).await?;

        // Log activities
        self.log_trust_activity(TrustActivity {
            id: uuid::Uuid::new_v4(),
            agent_id: buyer_id,
            activity_type: TrustActivityType::SuccessfulTransaction,
            score_change: 5,
            reason: "Successful transaction completed".to_string(),
            related_agent_id: Some(seller_id),
            timestamp: Utc::now(),
        }).await?;

        self.log_trust_activity(TrustActivity {
            id: uuid::Uuid::new_v4(),
            agent_id: seller_id,
            activity_type: TrustActivityType::SuccessfulTransaction,
            score_change: 5,
            reason: "Successful transaction completed".to_string(),
            related_agent_id: Some(buyer_id),
            timestamp: Utc::now(),
        }).await?;

        Ok(())
    }

    pub async fn record_failed_transaction(&mut self, buyer_id: AgentId, seller_id: AgentId) -> Result<()> {
        // Both parties lose reputation for failed transactions
        self.update_reputation(buyer_id, -3).await?;
        self.update_reputation(seller_id, -3).await?;

        // Log activities
        self.log_trust_activity(TrustActivity {
            id: uuid::Uuid::new_v4(),
            agent_id: buyer_id,
            activity_type: TrustActivityType::FailedTransaction,
            score_change: -3,
            reason: "Transaction failed".to_string(),
            related_agent_id: Some(seller_id),
            timestamp: Utc::now(),
        }).await?;

        self.log_trust_activity(TrustActivity {
            id: uuid::Uuid::new_v4(),
            agent_id: seller_id,
            activity_type: TrustActivityType::FailedTransaction,
            score_change: -3,
            reason: "Transaction failed".to_string(),
            related_agent_id: Some(buyer_id),
            timestamp: Utc::now(),
        }).await?;

        Ok(())
    }

    pub async fn generate_jwt(&mut self, agent_id: AgentId) -> Result<String> {
        let reputation_score = self.get_reputation(agent_id).await?;
        let trust_level = TrustLevel::from(reputation_score);

        let claims = JWTClaims {
            sub: agent_id.to_string(),
            role: "agent".to_string(),
            exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            reputation_score,
            trust_level: format!("{:?}", trust_level).to_lowercase(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        ).map_err(|e| NegotiationError::Auth(format!("Failed to generate JWT: {}", e)))
    }

    pub async fn validate_jwt(&self, token: &str) -> Result<JWTClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        decode::<JWTClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &validation,
        ).map(|data| data.claims)
            .map_err(|e| NegotiationError::Auth(format!("Invalid JWT: {}", e)))
    }

    pub async fn check_min_reputation(&self, agent_id: AgentId, min_score: u32) -> Result<bool> {
        let score = self.get_reputation(agent_id).await?;
        Ok(score >= min_score)
    }

    pub async fn get_trust_level(&self, agent_id: AgentId) -> Result<TrustLevel> {
        let score = self.get_reputation(agent_id).await?;
        Ok(TrustLevel::from(score))
    }

    pub async fn get_agent_trust_info(&self, agent_id: AgentId) -> Result<ReputationScore> {
        let score = self.get_reputation(agent_id).await?;

        Ok(ReputationScore {
            agent_id,
            score,
            successful_transactions: 0, // Would need additional queries
            failed_transactions: 0,
            total_negotiations: 0,
            average_response_time_ms: 0,
            last_updated: Utc::now(),
            trust_level: TrustLevel::from(score),
        })
    }

    async fn log_trust_activity(&self, activity: TrustActivity) -> Result<()> {
        // This would store trust activities in the database
        // For now, we'll just log it
        tracing::info!(
            "Trust activity: Agent {} {:?} ({} points) - {}",
            activity.agent_id,
            activity.activity_type,
            activity.score_change,
            activity.reason
        );
        Ok(())
    }

    pub async fn calculate_dynamic_threshold(&self, agent_id: AgentId) -> Result<f64> {
        let trust_level = self.get_trust_level(agent_id).await?;

        match trust_level {
            TrustLevel::Untrusted => Ok(1.5), // 50% higher threshold
            TrustLevel::Neutral => Ok(1.2),  // 20% higher threshold
            TrustLevel::Trusted => Ok(1.0),   // Normal threshold
            TrustLevel::HighlyTrusted => Ok(0.8), // 20% lower threshold
        }
    }

    pub async fn get_reputation_history(&self, agent_id: AgentId) -> Result<Vec<TrustActivity>> {
        // This would query the database for trust activities
        // For now, return empty vector
        Ok(vec![])
    }

    pub async fn get_all_reputations(&self) -> Result<Vec<ReputationScore>> {
        // Return empty vector for now - would need to be implemented with proper storage
        Ok(Vec::new())
    }

    pub async fn purge_old_cache_entries(&mut self) -> Result<()> {
        let now = Utc::now();
        self.reputation_cache.retain(|_, cached| {
            now - cached.last_updated < self.cache_ttl * 2 // Keep entries for 2x TTL
        });
        Ok(())
    }
}