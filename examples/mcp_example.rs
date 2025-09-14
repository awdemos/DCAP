//! MCP (Model Context Protocol) Example
//!
//! This example demonstrates how to use the MCP server for LLM-to-LLM negotiation.

use std::io::{self, Read, Write};
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Negotiation Agents MCP Example");
    println!("==============================");

    // Connect to MCP server
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;
    println!("Connected to MCP server");

    // Example 1: Search for agents
    println!("\n1. Searching for electronics agents...");
    let search_request = serde_json::json!({
        "id": "search-001",
        "method": "tools/call",
        "params": {
            "name": "search_agents",
            "arguments": {
                "category": "Electronics",
                "min_reputation": 50
            }
        }
    });

    send_request(&mut stream, &search_request)?;
    let response = read_response(&mut stream)?;
    println!("Search Response: {}", serde_json::to_string_pretty(&response)?);

    // Example 2: Get product catalog
    println!("\n2. Getting product catalog...");
    let catalog_request = serde_json::json!({
        "id": "catalog-001",
        "method": "resources/read",
        "params": {
            "uri": "product://catalog"
        }
    });

    send_request(&mut stream, &catalog_request)?;
    let response = read_response(&mut stream)?;
    println!("Catalog Response: {}", serde_json::to_string_pretty(&response)?);

    // Example 3: Get negotiation strategy prompt
    println!("\n3. Getting negotiation strategy prompt...");
    let prompt_request = serde_json::json!({
        "id": "prompt-001",
        "method": "prompts/get",
        "params": {
            "name": "negotiation_strategy"
        }
    });

    send_request(&mut stream, &prompt_request)?;
    let response = read_response(&mut stream)?;
    println!("Prompt Response: {}", serde_json::to_string_pretty(&response)?);

    // Example 4: Register a new agent
    println!("\n4. Registering a new agent...");
    let register_request = serde_json::json!({
        "id": "register-001",
        "method": "tools/call",
        "params": {
            "name": "register_agent",
            "arguments": {
                "agent_type": "seller",
                "name": "MCP Example Seller",
                "endpoint": "http://localhost:8001",
                "public_key": "example_public_key",
                "payment_methods": ["stripe"]
            }
        }
    });

    send_request(&mut stream, &register_request)?;
    let response = read_response(&mut stream)?;
    println!("Register Response: {}", serde_json::to_string_pretty(&response)?);

    println!("\nMCP Example completed!");
    Ok(())
}

fn send_request(stream: &mut TcpStream, request: &serde_json::Value) -> io::Result<()> {
    let request_str = serde_json::to_string(request)?;
    stream.write_all(request_str.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn read_response(stream: &mut TcpStream) -> io::Result<serde_json::Value> {
    let mut buffer = Vec::new();
    let mut temp_buffer = [0; 1024];

    loop {
        let bytes_read = stream.read(&mut temp_buffer)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp_buffer[..bytes_read]);

        // Check if we have a complete JSON response
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&buffer) {
            return Ok(json);
        }
    }

    Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete response"))
}