use crate::{model::*, AgentId, NegotiationError, Result, TransactionId};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::str::FromStr;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::from_str(database_url)?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        ).await?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                agent_type TEXT NOT NULL,
                name TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                public_key TEXT NOT NULL,
                reputation_score INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME NOT NULL,
                last_active DATETIME NOT NULL
            );

            CREATE TABLE IF NOT EXISTS products (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                base_price REAL NOT NULL,
                currency TEXT NOT NULL,
                stock_quantity INTEGER NOT NULL,
                metadata TEXT,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS negotiations (
                id TEXT PRIMARY KEY,
                rfq_id TEXT NOT NULL UNIQUE,
                quote_id TEXT,
                buyer_id TEXT NOT NULL,
                seller_id TEXT NOT NULL,
                product_id TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                opening_bid REAL NOT NULL,
                close_price REAL,
                delta REAL,
                status TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL,
                FOREIGN KEY (buyer_id) REFERENCES agents(id),
                FOREIGN KEY (seller_id) REFERENCES agents(id),
                FOREIGN KEY (quote_id) REFERENCES quotes(id)
            );

            CREATE TABLE IF NOT EXISTS quotes (
                id TEXT PRIMARY KEY,
                rfq_id TEXT NOT NULL,
                seller_id TEXT NOT NULL,
                price REAL NOT NULL,
                currency TEXT NOT NULL,
                available_quantity INTEGER NOT NULL,
                delivery_estimate TEXT,
                ttl_seconds INTEGER NOT NULL,
                metadata TEXT,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (rfq_id) REFERENCES negotiations(rfq_id),
                FOREIGN KEY (seller_id) REFERENCES agents(id)
            );

            CREATE TABLE IF NOT EXISTS negotiation_messages (
                id TEXT PRIMARY KEY,
                negotiation_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                content TEXT NOT NULL,
                message_type TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (negotiation_id) REFERENCES negotiations(id) ON DELETE CASCADE,
                FOREIGN KEY (sender_id) REFERENCES agents(id)
            );

            CREATE TABLE IF NOT EXISTS negotiation_records (
                buyer_id TEXT NOT NULL,
                seller_id TEXT NOT NULL,
                product_hash TEXT NOT NULL,
                opening_bid REAL NOT NULL,
                close_price REAL NOT NULL,
                delta REAL NOT NULL,
                timestamp DATETIME NOT NULL,
                duration_seconds INTEGER NOT NULL,
                message_count INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_agents_type ON agents(agent_type);
            CREATE INDEX IF NOT EXISTS idx_agents_reputation ON agents(reputation_score DESC);
            CREATE INDEX IF NOT EXISTS idx_products_agent ON products(agent_id);
            CREATE INDEX IF NOT EXISTS idx_negotiations_status ON negotiations(status);
            CREATE INDEX IF NOT EXISTS idx_negotiations_buyer ON negotiations(buyer_id);
            CREATE INDEX IF NOT EXISTS idx_negotiations_seller ON negotiations(seller_id);
            CREATE INDEX IF NOT EXISTS idx_quotes_seller ON quotes(seller_id);
            CREATE INDEX IF NOT EXISTS idx_records_timestamp ON negotiation_records(timestamp);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_agent(&self, agent: &AgentInfo) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO agents (id, agent_type, name, endpoint, public_key, reputation_score, created_at, last_active)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(agent.id.to_string())
        .bind(format!("{:?}", agent.agent_type))
        .bind(&agent.name)
        .bind(&agent.endpoint)
        .bind(&agent.public_key)
        .bind(agent.reputation_score)
        .bind(agent.created_at)
        .bind(agent.last_active)
        .execute(&self.pool)
        .await?;

        for product in &agent.products {
            self.create_product(product, agent.id).await?;
        }

        Ok(())
    }

    pub async fn create_product(&self, product: &Product, agent_id: AgentId) -> Result<()> {
        let metadata = serde_json::to_string(&product.metadata)?;
        sqlx::query(
            r#"
            INSERT INTO products (id, agent_id, name, description, category, base_price, currency, stock_quantity, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&product.id)
        .bind(agent_id.to_string())
        .bind(&product.name)
        .bind(&product.description)
        .bind(&product.category)
        .bind(product.base_price)
        .bind(&product.currency)
        .bind(product.stock_quantity)
        .bind(metadata)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_agent(&self, agent_id: AgentId) -> Result<Option<AgentInfo>> {
        let row = sqlx::query(
            r#"
            SELECT id, agent_type, name, endpoint, public_key, reputation_score, created_at, last_active
            FROM agents WHERE id = ?
            "#,
        )
        .bind(agent_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let agent_type = match row.get::<_, String>(1).as_str() {
                    "Buyer" => AgentType::Buyer,
                    "Seller" => AgentType::Seller,
                    _ => return Err(NegotiationError::Validation("Invalid agent type".to_string())),
                };

                let agent = AgentInfo {
                    id: AgentId::parse_str(&row.get::<_, String>(0))?,
                    agent_type,
                    name: row.get(2),
                    endpoint: row.get(3),
                    public_key: row.get(4),
                    reputation_score: row.get(5),
                    created_at: row.get(6),
                    last_active: row.get(7),
                    products: vec![],
                    payment_methods: vec![],
                };

                Ok(Some(agent))
            }
            None => Ok(None),
        }
    }

    pub async fn get_agents_by_type(&self, agent_type: AgentType) -> Result<Vec<AgentInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT id, agent_type, name, endpoint, public_key, reputation_score, created_at, last_active
            FROM agents WHERE agent_type = ? ORDER BY reputation_score DESC
            "#,
        )
        .bind(format!("{:?}", agent_type))
        .fetch_all(&self.pool)
        .await?;

        let mut agents = Vec::new();
        for row in rows {
            let agent_type = match row.get::<_, String>(1).as_str() {
                "Buyer" => AgentType::Buyer,
                "Seller" => AgentType::Seller,
                _ => return Err(NegotiationError::Validation("Invalid agent type".to_string())),
            };

            agents.push(AgentInfo {
                id: AgentId::parse_str(&row.get::<_, String>(0))?,
                agent_type,
                name: row.get(2),
                endpoint: row.get(3),
                public_key: row.get(4),
                reputation_score: row.get(5),
                created_at: row.get(6),
                last_active: row.get(7),
                products: vec![],
                payment_methods: vec![],
            });
        }

        Ok(agents)
    }

    pub async fn create_negotiation(&self, negotiation: &Negotiation) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO negotiations (id, rfq_id, quote_id, buyer_id, seller_id, product_id, quantity, opening_bid, close_price, delta, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(negotiation.id.to_string())
        .bind(negotiation.rfq_id.to_string())
        .bind(negotiation.quote_id.map(|id| id.to_string()))
        .bind(negotiation.buyer_id.to_string())
        .bind(negotiation.seller_id.to_string())
        .bind(&negotiation.product_id)
        .bind(negotiation.quantity)
        .bind(negotiation.opening_bid)
        .bind(negotiation.close_price)
        .bind(negotiation.delta)
        .bind(format!("{:?}", negotiation.status))
        .bind(negotiation.created_at)
        .bind(negotiation.updated_at)
        .execute(&self.pool)
        .await?;

        for message in &negotiation.messages {
            self.create_negotiation_message(message).await?;
        }

        Ok(())
    }

    pub async fn create_negotiation_message(&self, message: &NegotiationMessage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO negotiation_messages (id, negotiation_id, sender_id, content, message_type, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(message.id.to_string())
        .bind(message.negotiation_id.to_string())
        .bind(message.sender_id.to_string())
        .bind(&message.content)
        .bind(format!("{:?}", message.message_type))
        .bind(message.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_quote(&self, quote: &Quote) -> Result<()> {
        let metadata = serde_json::to_string(&quote.metadata)?;
        sqlx::query(
            r#"
            INSERT INTO quotes (id, rfq_id, seller_id, price, currency, available_quantity, delivery_estimate, ttl_seconds, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(quote.id.to_string())
        .bind(quote.rfq_id.to_string())
        .bind(quote.seller_id.to_string())
        .bind(quote.price)
        .bind(&quote.currency)
        .bind(quote.available_quantity)
        .bind(&quote.delivery_estimate)
        .bind(quote.ttl_seconds)
        .bind(metadata)
        .bind(quote.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_negotiation(&self, negotiation_id: TransactionId) -> Result<Option<Negotiation>> {
        let row = sqlx::query(
            r#"
            SELECT id, rfq_id, quote_id, buyer_id, seller_id, product_id, quantity, opening_bid, close_price, delta, status, created_at, updated_at
            FROM negotiations WHERE id = ?
            "#,
        )
        .bind(negotiation_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let status = match row.get::<_, String>(10).as_str() {
                    "pending" => NegotiationStatus::Pending,
                    "quoted" => NegotiationStatus::Quoted,
                    "negotiating" => NegotiationStatus::Negotiating,
                    "accepted" => NegotiationStatus::Accepted,
                    "rejected" => NegotiationStatus::Rejected,
                    "expired" => NegotiationStatus::Expired,
                    "settled" => NegotiationStatus::Settled,
                    _ => return Err(NegotiationError::Validation("Invalid negotiation status".to_string())),
                };

                let negotiation = Negotiation {
                    id: TransactionId::parse_str(&row.get::<_, String>(0))?,
                    rfq_id: TransactionId::parse_str(&row.get::<_, String>(1))?,
                    quote_id: row.get::<_, Option<String>>(2).map(|s| TransactionId::parse_str(&s)).transpose()?,
                    buyer_id: AgentId::parse_str(&row.get::<_, String>(3))?,
                    seller_id: AgentId::parse_str(&row.get::<_, String>(4))?,
                    product_id: row.get(5),
                    quantity: row.get(6),
                    opening_bid: row.get(7),
                    close_price: row.get(8),
                    delta: row.get(9),
                    status,
                    messages: vec![],
                    created_at: row.get(11),
                    updated_at: row.get(12),
                };

                Ok(Some(negotiation))
            }
            None => Ok(None),
        }
    }

    pub async fn update_negotiation(&self, negotiation: &Negotiation) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE negotiations
            SET quote_id = ?, close_price = ?, delta = ?, status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(negotiation.quote_id.map(|id| id.to_string()))
        .bind(negotiation.close_price)
        .bind(negotiation.delta)
        .bind(format!("{:?}", negotiation.status))
        .bind(negotiation.updated_at)
        .bind(negotiation.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_negotiation_record(&self, record: &NegotiationRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO negotiation_records (buyer_id, seller_id, product_hash, opening_bid, close_price, delta, timestamp, duration_seconds, message_count)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(record.buyer_id.to_string())
        .bind(record.seller_id.to_string())
        .bind(&record.product_hash)
        .bind(record.opening_bid)
        .bind(record.close_price)
        .bind(record.delta)
        .bind(record.timestamp)
        .bind(record.duration_seconds)
        .bind(record.message_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_negotiation_records(&self, limit: i64) -> Result<Vec<NegotiationRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT buyer_id, seller_id, product_hash, opening_bid, close_price, delta, timestamp, duration_seconds, message_count
            FROM negotiation_records ORDER BY timestamp DESC LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::new();
        for row in rows {
            records.push(NegotiationRecord {
                buyer_id: AgentId::parse_str(&row.get::<_, String>(0))?,
                seller_id: AgentId::parse_str(&row.get::<_, String>(1))?,
                product_hash: row.get(2),
                opening_bid: row.get(3),
                close_price: row.get(4),
                delta: row.get(5),
                timestamp: row.get(6),
                duration_seconds: row.get(7),
                message_count: row.get(8),
            });
        }

        Ok(records)
    }

    pub async fn update_agent_reputation(&self, agent_id: AgentId, score_change: i32) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE agents SET reputation_score = reputation_score + ? WHERE id = ?
            "#,
        )
        .bind(score_change)
        .bind(agent_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_agent_reputation(&self, agent_id: AgentId) -> Result<u32> {
        let row = sqlx::query(
            r#"
            SELECT reputation_score FROM agents WHERE id = ?
            "#,
        )
        .bind(agent_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get(0))
    }
}