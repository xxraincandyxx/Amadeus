//! # HTTP Server Example - SDK Test Harness
//!
//! This is a test HTTP server for the Amadeus SDK.
//! It demonstrates SDK usage via HTTP API.

use anyhow::Result;

use amadeus::api::http::run_server;

#[tokio::main]
async fn main() -> Result<()> {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    
    println!("Starting Amadeus SDK HTTP server on port {}...", port);
    run_server(port).await?;
    
    Ok(())
}
