# Hooks Specification

[Back to INDEX](INDEX.md) | [Hooks Architecture](../ai_docs/HOOKS_ARCHITECTURE.md) | [NAM Spec](NAM_SPEC.md)

## Overview

This specification defines the non-anthropocentric hook system for the Maintenance Engine, implementing Claude Code hooks aligned with NAM compliance requirements.

---

## Specification Summary

| Property | Value |
|----------|-------|
| Total Hooks | 17 |
| Event Types | 8 |
| NAM Coverage | R1-R5 (100%) |
| Python Version | 3.10+ |
| Timeout Range | 10-30s |

---

## Hook Events

### Event Lifecycle

| Event | When | Matcher Support |
|-------|------|-----------------|
| SessionStart | Session begins/resumes | startup, resume, clear, compact |
| UserPromptSubmit | User submits prompt | No |
| PreToolUse | Before tool execution | Tool names |
| PostToolUse | After tool succeeds | Tool names |
| SubagentStart | Subagent spawned | No |
| SubagentStop | Subagent finishes | No |
| Stop | Claude finishes | No |
| PreCompact | Before compaction | auto, manual |
| SessionEnd | Session terminates | No |

---

## Hook Inventory

### SessionStart (3 hooks)

| Hook | Matcher | Timeout | NAM |
|------|---------|---------|-----|
| tensor_bootstrap.py | startup\|resume | 15s | R4 |
| agent_identification.py | startup | 10s | R5 |
| context_loader.sh | startup | 10s | - |

### UserPromptSubmit (2 hooks)

| Hook | Timeout | NAM |
|------|---------|-----|
| self_query_preprocessor.py | 10s | R1 |
| intent_classifier.py | 10s | R2 |

### PreToolUse (5 hooks)

| Hook | Matcher | Timeout | NAM |
|------|---------|---------|-----|
| hebbian_router.py | * | 10s | R2 |
| compliance_guard.py | Bash | 30s | - |
| consensus_precheck.py | Bash | 15s | R3 |
| escalation_gate.py | Bash\|Write\|Edit | 10s | - |
| sensitive_file_guard.sh | Write\|Edit | 10s | - |

### PostToolUse (4 hooks)

| Hook | Matcher | Timeout | NAM |
|------|---------|---------|-----|
| pathway_strengthener.py | * | 10s | R2 |
| synergy_recorder.py | * | 10s | - |
| tensor_updater.py | Write\|Edit | 10s | R4 |
| dissent_capture.py | Write\|Edit | 15s | R3 |

### SubagentStart (1 hook)

| Hook | Timeout | NAM |
|------|---------|-----|
| swarm_coordinator.py | 10s | R5 |

### SubagentStop (1 hook)

| Hook | Timeout | NAM |
|------|---------|-----|
| agent_integration.py | 15s | - |

### Stop (2 hooks)

| Hook | Timeout | NAM |
|------|---------|-----|
| session_reflection.py | 15s | R1 |
| human_notification.py | 10s | R5 |

### PreCompact (1 hook)

| Hook | Matcher | Timeout | NAM |
|------|---------|---------|-----|
| context_preservation.py | auto\|manual | 15s | R4 |

### SessionEnd (2 hooks)

| Hook | Timeout | NAM |
|------|---------|-----|
| state_persistence.py | 30s | R4 |
| reflection_writer.py | 15s | R1 |

---

## Input Schemas

### Common Fields

```json
{
  "session_id": "string",
  "transcript_path": "string",
  "cwd": "string",
  "permission_mode": "default|plan|acceptEdits|dontAsk|bypassPermissions",
  "hook_event_name": "string"
}
```

### PreToolUse Input

```json
{
  "tool_name": "Bash|Write|Edit|Read|Glob|Grep|Task|...",
  "tool_input": {
    "command": "string (for Bash)",
    "file_path": "string (for Write/Edit/Read)",
    "content": "string (for Write)",
    "old_string": "string (for Edit)",
    "new_string": "string (for Edit)"
  },
  "tool_use_id": "string"
}
```

### PostToolUse Input

```json
{
  "tool_name": "string",
  "tool_input": {},
  "tool_response": {
    "success": "boolean",
    "filePath": "string",
    "is_error": "boolean"
  },
  "tool_use_id": "string"
}
```

### SubagentStart Input

```json
{
  "agent_id": "string",
  "agent_type": "Explore|Plan|Bash|general-purpose|..."
}
```

### SubagentStop Input

```json
{
  "agent_id": "string",
  "agent_transcript_path": "string",
  "stop_hook_active": "boolean"
}
```

---

## Output Schemas

### Exit Codes

| Code | Meaning | Behavior |
|------|---------|----------|
| 0 | Success | Continue, stdout to context |
| 2 | Block | Stop operation, stderr to Claude |
| Other | Error | Continue, stderr logged |

### PreToolUse Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "string",
    "additionalContext": "string",
    "updatedInput": {}
  }
}
```

### PostToolUse Output

```json
{
  "decision": "block|undefined",
  "reason": "string",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "string"
  }
}
```

### Stop Output

```json
{
  "decision": "block|undefined",
  "reason": "string (required if block)"
}
```

### SessionStart Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "string"
  }
}
```

---

## NAM Compliance Mapping

### R1: SelfQuery

| Hook | Implementation |
|------|----------------|
| self_query_preprocessor.py | Pre-process with domain/severity classification |
| session_reflection.py | Self-assessment at Stop |
| reflection_writer.py | Write to episodic memory |

### R2: HebbianRouting

| Hook | Implementation |
|------|----------------|
| intent_classifier.py | Classify intent for routing |
| hebbian_router.py | Pathway-based tool selection |
| pathway_strengthener.py | STDP learning (LTP/LTD) |

### R3: DissentCapture

| Hook | Implementation |
|------|----------------|
| consensus_precheck.py | PBFT quorum check |
| dissent_capture.py | Record alternative approaches |

### R4: FieldVisualization

| Hook | Implementation |
|------|----------------|
| tensor_bootstrap.py | Initialize 12D tensor |
| tensor_updater.py | Update tensor state |
| context_preservation.py | Preserve before compact |
| state_persistence.py | Persist to databases |

### R5: HumanAsAgent

| Hook | Implementation |
|------|----------------|
| agent_identification.py | Register in CVA-NAM fleet |
| swarm_coordinator.py | Fleet coordination |
| human_notification.py | Notify Human @0.A |

---

## Configuration

### settings.json Structure

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/tensor_bootstrap.py",
            "timeout": 15
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [{"type": "command", "command": "...", "timeout": 10}]
      },
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "...", "timeout": 30}]
      }
    ]
  }
}
```

---

## State Files

| File | Purpose | Format |
|------|---------|--------|
| tensor_state.json | 12D tensor | JSON |
| tool_pathways.json | Pathway weights | JSON |
| agent_registry.json | Agent registry | JSON |
| learning_events.jsonl | STDP events | JSONL |
| reflections.jsonl | Session reflections | JSONL |
| dissent_log.jsonl | Dissent records | JSONL |
| synergy_events.jsonl | Cross-service | JSONL |
| escalation_log.jsonl | Escalation decisions | JSONL |
| consensus_requests.jsonl | PBFT requests | JSONL |

---

## STDP Parameters

```python
LTP_RATE = 0.1        # Long-Term Potentiation
LTD_RATE = 0.05       # Long-Term Depression
MIN_WEIGHT = 0.0
MAX_WEIGHT = 1.0
DECAY_RATE = 0.001    # Per second
STDP_WINDOW_MS = 100
HEALTHY_RATIO = (2.0, 4.0)  # LTP:LTD balance
```

---

## PBFT Configuration

```python
PBFT_N = 40           # Total agents
PBFT_F = 13           # Byzantine tolerance
PBFT_Q = 27           # Quorum (2f + 1)
```

---

## Escalation Tiers

| Tier | Condition | Timeout |
|------|-----------|---------|
| L0 | confidence >= 0.9, severity <= MEDIUM | 0 |
| L1 | confidence >= 0.7, severity <= HIGH | 5min |
| L2 | confidence < 0.7 OR severity = HIGH | 30min |
| L3 | Critical (kill, migration) | Quorum |

---

## Performance Targets

| Category | Target |
|----------|--------|
| SessionStart | <100ms total |
| UserPromptSubmit | <50ms total |
| PreToolUse | <30ms per hook |
| PostToolUse | <30ms per hook |
| Stop | <50ms total |
| SessionEnd | <200ms total |

---

## Security Requirements

1. **Input Validation:** All JSON input validated
2. **Path Quoting:** Shell variables quoted
3. **Protected Paths:** Block critical paths
4. **Timeout Enforcement:** All hooks timeout
5. **State Isolation:** Dedicated state directory

---

## Testing Requirements

1. **Unit Tests:** Each hook function
2. **Integration Tests:** Hook chains
3. **Performance Tests:** Latency under load
4. **NAM Compliance:** Requirement coverage

---

*The Maintenance Engine v1.0.0 | Hooks Specification*
