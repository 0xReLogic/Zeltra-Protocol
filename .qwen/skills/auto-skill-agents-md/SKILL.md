---
name: agents-md
description: Create lean AGENTS.md context files for AI agents working on codebases — behavior rules, invariants, environment setup, not todo copies
source: auto-skill
extracted_at: '2026-06-13T12:34:39.935Z'
---

# Creating Lean AGENTS.md Files

## Purpose
AGENTS.md provides context for AI agents entering a codebase. It should be **lean and focused** — not a verbose copy of TODO.md or project docs.

## What Worked

**Structure (in order):**
1. **One-liner project summary** — what this is, core purpose
2. **Repo layout** — directory structure with 1-line descriptions
3. **Core business flow** — ASCII diagram showing data/state flow
4. **Invariants** — hard rules that break the system if violated (accounting, crypto, state)
5. **Behavior rules** — before coding, while coding, while testing, secrets handling
6. **Environment & deployment** — how to source config, run the system
7. **Key files to read** — pointer to todo, decisions, business docs

**What to exclude:**
- Detailed TODO items (link to todo.md instead)
- Mock/insecure code listings (that's in todo.md)
- Research decisions (link to research/decisions/)
- Priority work lists (that's todo.md's job)

**Length target:** ~100-150 lines max. If it's growing, cut content that belongs elsewhere.

## Iteration Pattern

Start with comprehensive version → user feedback shows it's too verbose → trim to essentials → final lean version. The user wants agents to **read AGENTS.md first** then follow pointers, not get everything dumped upfront.

## Example Sections

**Lean guardian cluster section (7 lines):**
```markdown
## Guardian Cluster

\`\`\`bash
bash scripts/start_cluster.sh start    # 1 leader + 4 guardians + Vault
bash scripts/start_cluster.sh status   # check health
bash scripts/start_cluster.sh stop     # teardown
\`\`\`

Threshold 3/5. Auto-loads `.env.test` for RPC/contract/key. Health check timeout ≥15s.
```

**Verbose version (what didn't work):**
- Numbered step-by-step breakdown of what the script does internally
- Alternative manual approaches with full explanations
- Detailed health check reasoning

## Key Principle

**"Less is More"** — context pollution wastes tokens and confuses agents. Give them the **behavior framework** and **pointers to detail**, not the detail itself.