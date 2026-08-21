# Settings and troubleshooting

Open **Connection settings** from the sidebar footer or use `/settings`.

## Connection

The native app supervises its bundled local server. The web client connects to the configured HTTP API URL. Use **Test** to verify the address before saving it.

Remote servers should use HTTPS and authentication at the network boundary. Do not expose an unauthenticated local development server to an untrusted network.

## Language

Choose English or Simplified Chinese under **Interface language**. The app stores the selection locally and uses the matching guide manuscript.

## Common problems

### API unavailable

Confirm that the server is running and that the configured URL is reachable. In development, the default address is `http://127.0.0.1:3000`.

### Live connection interrupted

The event stream retries automatically. If history is present but live updates stop, test the connection and reconnect.

### Agent is waiting

Check for an approval card in the selected conversation. In multi-agent work, open Agents and look for an approval-required child.

### Context is nearly full

Run `/context`, then `/compact` if older conversation history can be summarized.
