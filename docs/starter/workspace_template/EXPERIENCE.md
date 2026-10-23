# EcoHarness Experience Log

This log stores episodic lessons adopted during Plan Mode (Propose -> Probe -> Compose) optimization passes. The agent retrieves the top-K relevant lessons for each task prompt using keyword ranking.

## Entry 1: Invoice Processing
- Task: Verify customer invoice and execute payment gateway charge
- Adopted Components: ToolGuidance
- Lesson: Always supply payment amounts as integer cents (`amount_cents`) instead of floating point strings to prevent gateway rejection.
