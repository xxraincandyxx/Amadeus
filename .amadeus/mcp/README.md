# MCP Guide

Amadeus currently has MCP client and adapter support in core, but MCP server configuration is not yet loaded from `.amadeus/settings.json`.

Current state:

- supported in code: `McpServerConfig`, `McpClient`, and MCP tool adapters
- not yet wired: project or user MCP server settings in `.amadeus/settings.json`

Current runtime types:

- [crates/core/src/mcp/client.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/mcp/client.rs)
- [crates/core/src/mcp/adapter.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/mcp/adapter.rs)

Current code-level server configuration:

```rust
use std::collections::HashMap;

use amadeus::mcp::{McpClient, McpServerConfig};

let config = McpServerConfig {
    command: "npx".to_string(),
    args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
    env: HashMap::new(),
};

let mut client = McpClient::connect(&config).await?;
let tools = client.list_tools().await?;
```

Planned settings direction:

- user/project/local MCP server definitions under `.amadeus`
- transport and lifecycle management
- discovery and degraded-mode reporting

Until that lands, treat this folder as documentation only.
