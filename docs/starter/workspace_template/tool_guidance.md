# Tool Guidance

Guidelines and best practices for invoking tools:

1. **Parameter Integrity:** Always verify that all required fields are present in `tool_input` before invoking a tool.
2. **Numeric Precision:** For payment and invoice processing tools, ensure monetary values are supplied as integer cents (`amount_cents`), not floating-point strings.
3. **JSON Structure:** Ensure tool input arguments are serialized as valid JSON strings.
