use dcap::{
    agent::{BuyerAgent, BuyerAgentConfig, LLMConfig},
    discovery::DiscoveryService,
    error::NegotiationError,
    settlement::SettlementService,
    trust::TrustSystem,
};
use clap::Parser;
use std::env;

#[derive(Parser)]
#[command(name = "buyer-agent")]
#[command(about = "LLM-powered buyer agent for marketplace negotiations")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short, long, default_value = "sqlite://negotiation.db")]
    database_url: String,

    #[arg(short, long, default_value = "http://localhost:8000")]
    discovery_endpoint: String,

    #[arg(short, long, default_value = "8002")]
    port: u16,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    let discovery = DiscoveryService::new(args.discovery_endpoint.clone());
    let trust = TrustSystem::new()?;
    let settlement_config = dcap::settlement::SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    };
    let settlement = SettlementService::new(settlement_config).await?;

    let buyer_config = BuyerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "TechBuyer".to_string(),
        endpoint: format!("http://localhost:{}", args.port),
        max_concurrent_negotiations: 5,
        default_ttl_hours: 24,
        llm_config: LLMConfig {
            model: "gpt-4".to_string(),
            api_key: env::var("OPENAI_API_KEY").unwrap_or_else(|_| "mock_key".to_string()),
            max_tokens: 1000,
            temperature: 0.7,
        },
    };

    let mut buyer_agent = BuyerAgent::new(
        buyer_config,
        discovery,
        trust,
        settlement,
    ).await?;

    println!("Buyer agent started on port {}", args.port);
    println!("Available commands:");
    println!("  browse [category] - Browse products");
    println!("  quote <product_id> <quantity> <max_price> - Request quote");
    println!("  negotiate <negotiation_id> <counter_offer> - Negotiate price");
    println!("  accept <negotiation_id> - Accept quote");
    println!("  reject <negotiation_id> - Reject quote");
    println!("  active - Show active negotiations");
    println!("  exit - Exit program");

    let mut input = String::new();
    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "exit" => break,
            "active" => {
                let negotiations = buyer_agent.get_active_negotiations();
                for neg in negotiations {
                    println!("Negotiation {}: Status: {:?}", neg.id, neg.status);
                }
            }
            cmd if cmd.starts_with("browse") => {
                let category = if cmd.len() > 7 {
                    Some(cmd[7..].trim().to_string())
                } else {
                    None
                };
                match buyer_agent.browse_products(category).await {
                    Ok(products) => {
                        println!("Found {} products:", products.len());
                        for product in products {
                            println!("  {} - ${} ({})", product.name, product.base_price, product.category);
                        }
                    }
                    Err(e) => println!("Error browsing products: {}", e),
                }
            }
            cmd if cmd.starts_with("quote") => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 4 {
                    let product_id = parts[1];
                    let quantity = parts[2].parse().unwrap_or(1);
                    let max_price = parts[3].parse().unwrap_or(0.0);

                    match buyer_agent.request_quote(product_id.to_string(), quantity, max_price).await {
                        Ok(negotiation_id) => println!("Quote requested. Negotiation ID: {}", negotiation_id),
                        Err(e) => println!("Error requesting quote: {}", e),
                    }
                } else {
                    println!("Usage: quote <product_id> <quantity> <max_price>");
                }
            }
            cmd if cmd.starts_with("negotiate") => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(negotiation_id) = uuid::Uuid::parse_str(parts[1]) {
                        let counter_offer = parts[2].parse().unwrap_or(0.0);

                        match buyer_agent.negotiate(negotiation_id, counter_offer).await {
                            Ok(()) => println!("Negotiation offer sent"),
                            Err(e) => println!("Error negotiating: {}", e),
                        }
                    } else {
                        println!("Invalid negotiation ID format");
                    }
                } else {
                    println!("Usage: negotiate <negotiation_id> <counter_offer>");
                }
            }
            cmd if cmd.starts_with("accept") => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(negotiation_id) = uuid::Uuid::parse_str(parts[1]) {
                        match buyer_agent.accept_quote(negotiation_id).await {
                            Ok(()) => println!("Quote accepted and payment processed"),
                            Err(e) => println!("Error accepting quote: {}", e),
                        }
                    } else {
                        println!("Invalid negotiation ID format");
                    }
                } else {
                    println!("Usage: accept <negotiation_id>");
                }
            }
            cmd if cmd.starts_with("reject") => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(negotiation_id) = uuid::Uuid::parse_str(parts[1]) {
                        match buyer_agent.reject_quote(negotiation_id).await {
                            Ok(()) => println!("Quote rejected"),
                            Err(e) => println!("Error rejecting quote: {}", e),
                        }
                    } else {
                        println!("Invalid negotiation ID format");
                    }
                } else {
                    println!("Usage: reject <negotiation_id>");
                }
            }
            "" => continue,
            _ => println!("Unknown command. Type 'help' for available commands."),
        }
    }

    println!("Buyer agent shutting down");
    Ok(())
}