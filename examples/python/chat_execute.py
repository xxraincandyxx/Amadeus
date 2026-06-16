# @amadeus-header
# summary: Python example for stateless chat and direct command execution endpoints.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/python/chat_execute.py
# uses:
# - module: examples.python.amadeus_client
# - protocol: Amadeus HTTP API
# invariants:
# - Example code remains dependency-free and runnable with Python 3.
# side_effects:
# - Performs network or HTTP operations.
# - Writes output to stdout or stderr.
# tests:
# - cmd: python3 examples/python/chat_execute.py --help
# @end-amadeus-header

from amadeus_client import AmadeusClient, parser, print_json


def main() -> None:
    arg_parser = parser("Use Amadeus /chat and /execute from Python.")
    arg_parser.add_argument("--message", default="Say hello in one short sentence.")
    arg_parser.add_argument("--command", default="pwd")
    args = arg_parser.parse_args()

    client = AmadeusClient(args.base_url)

    print("# Chat")
    print_json(client.post("/chat", {"message": args.message}))

    print("# Execute")
    print_json(client.post("/execute", {"command": args.command, "timeout_secs": 30}))


if __name__ == "__main__":
    main()
