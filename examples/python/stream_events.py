# @amadeus-header
# summary: Python example for consuming Amadeus Server-Sent Events.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/python/stream_events.py
# uses:
# - module: examples.python.amadeus_client
# - protocol: Amadeus HTTP API
# invariants:
# - Example code remains dependency-free and runnable with Python 3.
# side_effects:
# - Performs network or HTTP operations.
# - Writes output to stdout or stderr.
# tests:
# - cmd: python3 examples/python/stream_events.py --help
# @end-amadeus-header

from amadeus_client import AmadeusClient, parser


def main() -> None:
    arg_parser = parser("Stream Amadeus agent events from Python.")
    arg_parser.add_argument("--message", default="Summarize this project in three bullets.")
    args = arg_parser.parse_args()

    client = AmadeusClient(args.base_url)
    session = client.post(
        "/v1/sessions",
        {"name": "python-stream-example", "profile": "default"},
    )
    session_id = session["id"]
    client.post(f"/v1/sessions/{session_id}/messages", {"content": args.message})
    client.stream(f"/v1/sessions/{session_id}/events", {})


if __name__ == "__main__":
    main()
