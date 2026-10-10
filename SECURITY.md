# Security Policy

## Supported versions

Amadeus is under active development on the `master` branch; security fixes are
applied to the latest released and development revisions only.

## Reporting a vulnerability

Please report security vulnerabilities privately via
[GitHub security advisories](https://github.com/xxraincandyxx/Amadeus/security/advisories/new)
("Report a vulnerability" on the [Security
tab](https://github.com/xxraincandyxx/Amadeus/security)). Do **not** open a
public GitHub issue for a security problem.

Include as much of the following as you can:

- The affected surface (desktop app, browser workspace, TUI, HTTP API, Python
  SDK) and the component or endpoint involved.
- Steps to reproduce, proof-of-concept, or affected request/response traces.
- The OS and Amadeus version, plus any relevant configuration (redact secrets).
- Your assessment of impact and severity.

You should receive a response within a few days. Please avoid public disclosure
until the issue has been addressed.

## Scope notes

- The HTTP API ships without authentication and with full CORS by design; it is
  intended for trusted internal use or deployment behind a reverse proxy.
  Running it on an untrusted network is out of scope for a vulnerability report
  — see the warning in [README.md](README.md#http-api).
- Never commit secrets: `.env`, `.pem`, and `.key` files, API keys in
  `.amadeus/settings.json`, or session logs containing credentials.
