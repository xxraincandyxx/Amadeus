# Amadeus Parity Status

Reference baseline: `refs/claw-code-parity/rust/PARITY.md`

Amadeus tracks parity as tested behavior, not as a count of tools or UI labels. A gap only moves out of this document when an automated test proves the behavior.

## Behavioral gaps

- Approval-driving scenario harness coverage is still being expanded.
- Bash safety still needs deeper command classification and permission enforcement.
- File tools need stronger permission-mode and edge-case coverage.
- MCP lifecycle behavior and UI accounting are not fully verified.
- Configuration hierarchy precedence needs stricter proof in tests.

## Verification rule

- Prefer manifest-backed integration tests and timeline assertions.
- Keep README and architecture claims aligned with passing tests in this repository.
