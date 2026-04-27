# @amadeus-header
# summary: Python example for agent management, skills, sessions, history, and approvals endpoints.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/python/agents_sessions.py
# uses:
# - module: examples.python.amadeus_client
# - protocol: Amadeus HTTP API
# invariants:
# - Example code remains dependency-free and runnable with Python 3.
# side_effects:
# - Performs network or HTTP operations.
# - Writes output to stdout or stderr.
# tests:
# - cmd: python3 examples/python/agents_sessions.py --help
# @end-amadeus-header

from amadeus_client import AmadeusClient, parser, print_json


def main() -> None:
    args = parser("Inspect Amadeus management endpoints from Python.").parse_args()
    client = AmadeusClient(args.base_url)

    print("# Skills")
    print_json(client.get("/skills"))

    print("# Sessions")
    print_json(client.get("/sessions"))

    print("# History")
    print_json(client.get("/history"))

    print("# Pending approvals")
    print_json(client.get("/approvals"))

    print("# Agents before create")
    print_json(client.get("/agents"))

    print("# Create docs agent")
    created = client.post("/agents", {"name": "python-docs-example", "profile": "docs"})
    print_json(created)

    agent_id = created["agent"]["id"]
    print("# Chat with created agent")
    print_json(
        client.post(
            f"/agents/{agent_id}/chat",
            {"message": "Say hello from the Python agent example."},
        )
    )


if __name__ == "__main__":
    main()
