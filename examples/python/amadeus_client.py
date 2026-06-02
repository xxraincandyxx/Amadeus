# @amadeus-header
# summary: Small standard-library Python client shared by Amadeus HTTP API examples.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - module: examples.python.amadeus_client
# uses:
# - protocol: Amadeus HTTP API
# invariants:
# - Example code remains dependency-free and runnable with Python 3.
# side_effects:
# - Performs network or HTTP operations.
# tests:
# - cmd: python3 examples/python/health_config_tools.py --help
# @end-amadeus-header

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from typing import Any
from urllib import error, parse, request


@dataclass
class AmadeusClient:
    base_url: str = "http://127.0.0.1:3000"

    def get(self, path: str) -> dict[str, Any]:
        return self._request("GET", path)

    def post(self, path: str, payload: dict[str, Any]) -> dict[str, Any]:
        return self._request("POST", path, payload)

    def patch(self, path: str, payload: dict[str, Any]) -> dict[str, Any]:
        return self._request("PATCH", path, payload)

    def stream(self, path: str, params: dict[str, str]) -> None:
        query = parse.urlencode(params)
        url = f"{self.base_url.rstrip('/')}{path}?{query}"
        try:
            with request.urlopen(url) as response:
                for raw_line in response:
                    line = raw_line.decode("utf-8", errors="replace").rstrip()
                    if line:
                        print(line)
        except error.URLError as exc:
            raise SystemExit(f"Could not connect to {url}: {exc}") from exc

    def _request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        data = None
        headers = {"Accept": "application/json"}
        if payload is not None:
            data = json.dumps(payload).encode("utf-8")
            headers["Content-Type"] = "application/json"

        url = f"{self.base_url.rstrip('/')}{path}"
        req = request.Request(url, data=data, headers=headers, method=method)
        try:
            with request.urlopen(req) as response:
                body = response.read().decode("utf-8")
        except error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            raise SystemExit(f"{method} {url} failed with HTTP {exc.code}: {body}") from exc
        except error.URLError as exc:
            raise SystemExit(f"Could not connect to {url}: {exc}") from exc

        if not body:
            return {}
        return json.loads(body)


def parser(description: str) -> argparse.ArgumentParser:
    arg_parser = argparse.ArgumentParser(description=description)
    arg_parser.add_argument(
        "--base-url",
        default="http://127.0.0.1:3000",
        help="Amadeus server base URL",
    )
    return arg_parser


def print_json(value: Any) -> None:
    json.dump(value, sys.stdout, indent=2, sort_keys=True)
    print()
