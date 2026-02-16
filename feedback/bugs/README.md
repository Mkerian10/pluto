# MCP / AI-Native Workflow Feedback

This directory captures feedback on the experience of using Pluto's MCP tools for AI-agent workflows. This is distinct from language feedback — it's about the **tooling layer** between agents and the compiler.

## What Belongs Here

- How well MCP tools work for common agent tasks (reading code, editing, compiling, testing)
- Comparisons between MCP-based workflows and traditional file read/compile workflows
- Missing tools or capabilities that would improve the agent experience
- Tool ergonomics: parameter naming, output format, error messages
- Anything about the AI-native development experience

## Format

**File:** `mcp/<short-description>.md`

```markdown
# <Title>

**Project:** <which project>
**Date:** <YYYY-MM-DD>
**Tool(s):** <which MCP tool(s) involved>
**Type:** strength | friction | bug

## What Happened
Describe the workflow and how MCP tools were used (or attempted).

## Assessment
What worked well or what was painful about the MCP-based workflow vs traditional file/compile approach.

## Suggestion (if any)
How the tool could be improved.
```

## Rules

1. **File feedback immediately** — don't batch it up.
2. **One item per file.**
3. **Be specific** — name the exact tool(s) and describe the exact workflow.
4. **Include context** — what task were you trying to accomplish? What did you try first?
