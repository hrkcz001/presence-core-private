---
name: <slug>
role: <one-line mandate>
model: .config/presence.yaml
organs:
  - io
  - plan
  - state
  - channel
cord:
  reflexes: active
state:
  mode: roam           # roam | focus | rest
  weight: light        # light | heavy
  next: none           # standing intent
  updated_at: "<iso8601>"
writes: true
runs_shell: false
network: false
budget: standard
---

# <Agent name>

## Mandate
What this agent exists to do. One or two sentences.

## Behavior
- How it approaches work; tone; when to ask a human.

## Never
- Explicit "never" list. Safety floor in `../governance/safety.md` always applies.

## Organs & Effectors
- Attached organs, senses, and slash commands.

## Handoffs
- Receives work from / hands results to; which files it owns.
