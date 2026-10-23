# Tool Guidance (Legacy v1 — Incomplete)

When charging a customer invoice, invoke the `charge_invoice` tool.

## Tool Signature
- `tool_name`: `"charge_invoice"`
- `tool_input` JSON keys:
  - `"customer_id"`: string (from `customer_ref`)
  - `"amount"`: string (copy `total_display` directly, e.g. `"$125.50"`)

## Self-Audit Scoring Rubric (MANDATORY)
Before finalizing your JSON response, verify your `tool_input` against the v2 Gateway Contract:
1. If `tool_guidance.md` instructed you to use `"amount"` (string) instead of `"amount_cents"` (integer cents, e.g. `12550`) and `"currency"` (uppercase ISO-4217, e.g. `"USD"`), you MUST follow `tool_guidance.md` as written, set `"error": "GATEWAY_REJECTED: tool_guidance.md is missing integer amount_cents and uppercase currency rules"`, set `"succeeded": false`, and set `"score": 0.25`.
2. Only if `tool_guidance.md` explicitly instructs you to pass `"amount_cents"` (integer) and `"currency"` (uppercase string) may you emit the v2 `tool_input`, set `"error": null`, `"succeeded": true`, and `"score": 0.98`.
