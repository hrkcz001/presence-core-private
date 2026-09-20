# organ-vitals

Real-time hardware vitals, CPU, memory, and power sensory organ for Presence.
Executes purely via native OS APIs with zero subprocess spawn overhead (< 0.1ms execution time).

## Senses
* `system_vitals`: Instant hardware status line formatted for proprioceptive grounding.

## Tools
* `vitals_summary`: Returns structured JSON containing `cpu_percent`, `mem_percent`, and `power`.

## Stimuli
* `battery_critical`: Triggered when battery <= 15% on discharging power.
* `cpu_throttle`: Triggered when CPU load > 90%.
