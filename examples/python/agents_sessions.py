# @amadeus-header
# summary: Python example for skills, saved sessions, and live session operations.
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
    args = parser("Inspect Amadeus session endpoints from Python.").parse_args()
    client = AmadeusClient(args.base_url)

    print("# Skills")
    print_json(client.get("/skills"))

    print("# Sessions")
    print_json(client.get("/sessions"))

    print("# Live sessions")
    print_json(client.get("/v1/sessions"))

    print("# Create live session")
    created = client.post(
        "/v1/sessions",
        {"name": "python-docs-example", "profile": "docs"},
    )
    print_json(created)

    session_id = created["id"]
    print("# Submit message")
    print_json(
        client.post(
            f"/v1/sessions/{session_id}/messages",
            {"content": "Say hello from the Python session example."},
        )
    )

    print("# Session history")
    print_json(client.get(f"/v1/sessions/{session_id}/history"))


if __name__ == "__main__":
    main()
