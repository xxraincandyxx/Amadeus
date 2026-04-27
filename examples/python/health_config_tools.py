# @amadeus-header
# summary: Python example that inspects health, config, prompt profile, and tool inventory.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/python/health_config_tools.py
# uses:
# - module: examples.python.amadeus_client
# - protocol: Amadeus HTTP API
# invariants:
# - Example code remains dependency-free and runnable with Python 3.
# side_effects:
# - Performs network or HTTP operations.
# - Writes output to stdout or stderr.
# tests:
# - cmd: python3 examples/python/health_config_tools.py --help
# @end-amadeus-header

from amadeus_client import AmadeusClient, parser, print_json


def main() -> None:
    args = parser("Inspect Amadeus health, config, prompt profile, and tools.").parse_args()
    client = AmadeusClient(args.base_url)

    print("# Health")
    print_json(client.get("/health"))

    print("# Config summary")
    config = client.get("/config")
    print_json(
        {
            "model": config["model"],
            "working_dir": config["working_dir"],
            "prompt": config["prompt"],
            "tool_profile": config["tools"]["active_profile"],
            "tool_count": len(config["tools"]["inventory"]),
        }
    )

    print("# First configured tools")
    for tool in config["tools"]["inventory"][:10]:
        print_json(tool)


if __name__ == "__main__":
    main()
