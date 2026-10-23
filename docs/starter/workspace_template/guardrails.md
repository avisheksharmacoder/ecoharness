# Guardrails Policy

This file defines runtime security boundaries enforced by GuardrailEnforcer before any subprocess is created.

## Denied Tools
- DENY_TOOL: raw_shell
- DENY_TOOL: format_filesystem

## Forbidden Argument Substrings
- DENY_ARG_PATTERN: rm -rf
- DENY_ARG_PATTERN: DROP TABLE
- DENY_ARG_PATTERN: /dev/null

*(Path traversal attempts containing `../` or `..\` are unconditionally blocked by default).*
