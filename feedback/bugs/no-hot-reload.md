# MCP Server Cannot Be Hot-Reloaded Mid-Session

**Project:** Meridian
**Date:** 2026-02-10
**Tool(s):** All MCP tools
**Type:** friction

## What Happened

After diagnosing the stale MCP binary issue (see stale-binary-no-ci.md), I rebuilt the MCP server with `cargo build --release -p pluto-mcp`. The new binary was written to disk, but the running MCP server process (started when the Claude Code session began) continued using the old code. All MCP tool calls still returned the same parse errors.

The only way to pick up the new binary is to **restart the entire Claude Code session**, losing conversation context and requiring the agent to re-orient.

## Assessment

This is a significant friction point for iterative development on the MCP tooling itself. The workflow looks like:

1. Discover MCP bug/staleness
2. Fix the issue and rebuild the binary
3. **Cannot verify the fix** — MCP tools still use the old process
4. Must tell the user to restart the session
5. Agent loses all context from the debugging session
6. Start over, hope the fix worked

This makes the MCP server difficult to develop and debug from within the agent workflow that it's supposed to support.

## Suggestion

1. **Per-invocation mode**: Instead of a long-running stdio server, support a mode where each MCP tool call spawns a fresh process (like `cargo run`). Slower but always up-to-date. Useful during development.
2. **SIGHUP reload**: The MCP server could watch its own binary and re-exec on change, or respond to a signal by reloading.
3. **Reload tool**: Add a `reload_server` MCP tool that causes the server to re-exec itself from the same binary path, picking up changes.
4. **Version check tool**: At minimum, add a `server_info` tool that returns the build timestamp/git hash so agents can detect staleness and advise the user to restart.
