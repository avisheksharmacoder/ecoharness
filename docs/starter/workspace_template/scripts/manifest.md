# Scripts Manifest

Executable local CLI subprocess tools available to the agent:

```json
{
  "tools": [
    {
      "name": "system_time",
      "description": "Returns the current UTC ISO timestamp from the local host system.",
      "program": "date",
      "args": ["-u", "+%Y-%m-%dT%H:%M:%SZ"]
    },
    {
      "name": "calc",
      "description": "Evaluates a basic arithmetic expression provided via input.",
      "program": "python3",
      "args": ["-c", "import os; print(eval(os.environ.get('HARNESS_TOOL_INPUT', '0')))"]
    }
  ]
}
```
