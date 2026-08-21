# AI-Native Pipeline: Completion Plan

> **Status:** Implementation Plan
> **Created:** 2026-02-14
> **Target:** Complete MCP server for Claude Code integration
> **Estimated Duration:** 2-3 weeks

## Overview

The foundation for AI-native representation is complete (PLTO format, UUIDs, cross-references, SDK). This plan outlines the remaining work to deliver a production-ready MCP server that enables Claude Code to work with Pluto projects natively.

**Current state:** MCP server with 6 read-only tools working on single modules
**Target state:** Full-featured MCP server with project-awareness, write operations, and compile integration

---

## Phase 1: Project-Awareness (Week 1, Days 1-3)

### Goal
Transform the MCP server from module-at-a-time to project-aware, enabling cross-module queries and understanding of the full codebase.

### Tasks

#### 1.1: Project Index Structure
- [ ] Create `ProjectIndex` struct in `mcp/src/project.rs`
  - Scan project directory for all `.pluto` files (recursive)
  - Build module name → path mapping
  - Parse import declarations to build dependency graph
  - Detect project root (look for `main.pluto` or config file)
  - **Path safety:** Store canonical project root, normalize all paths relative to root
- [ ] Add `--project <path>` CLI flag to MCP server
  - Canonicalize project path on startup
  - Reject paths outside project root in all tools
- [ ] Initialize `ProjectIndex` on server startup, store in server state
- [ ] Add file watching (optional for Phase 1, required for Phase 5)
  - Detect external changes to `.pluto` files
  - Invalidate cache on external modifications
  - Notify agent of stale state (if needed)

**Success criteria:**
- Server can scan project directory and identify all modules
- Import graph is built correctly (with cycle detection)
- 5+ unit tests for project scanning and import resolution

#### 1.2: Cross-Module Declaration Search
- [ ] Implement `list_modules` tool
  - Returns all modules without loading each
  - Include metadata: path, name, pub declaration count
- [ ] Enhance `find_declaration` to search across all modules
  - Search by name across entire project
  - Optional kind filter and module filter
  - Return module path with each result
- [ ] Update server state to cache loaded modules by path
  - Lazy loading: only load on first access
  - Keep loaded modules in memory for performance

**Success criteria:**
- `list_modules` returns all modules in a multi-module test project
- `find_declaration` can locate declarations across module boundaries
- 10+ integration tests with multi-module test projects

#### 1.3: Cross-Module Cross-References
- [ ] Enhance `callers_of` to work across modules
  - Resolve import-qualified calls (e.g., `math.add()`)
  - Load caller modules as needed
- [ ] Enhance `usages_of` to search across modules
  - Type references in function signatures
  - Struct literals using imported classes
- [ ] Add `call_graph` tool with cross-module support
  - BFS/DFS traversal across module boundaries
  - Configurable max depth
  - Cycle detection

**Success criteria:**
- Cross-module call graph correctly traces through imports
- `callers_of` finds callers in importing modules
- 15+ tests for cross-module xref queries

#### 1.4: Import Resolution
- [ ] Implement import resolver
  - Map `import math` to `math.pluto` or `math/` directory
  - Handle hierarchical imports (e.g., `import std.collections`)
  - Support both single-file and directory modules
- [ ] Add import validation
  - Detect missing imports
  - Detect circular imports
  - Check `pub` visibility across module boundaries

**Success criteria:**
- Import resolver correctly finds modules in project
- Circular import detection works
- 8+ tests for import resolution edge cases

### Deliverables
- PR #1: Project-aware MCP server with cross-module queries
- 38+ tests total (5 + 10 + 15 + 8)
- Documentation: "MCP Server Architecture" doc explaining project state management

### Risk Mitigation
- **Performance:** Large projects may be slow to scan. Mitigation: benchmark with 50+ module project, add caching/incremental scanning if needed
- **Import complexity:** Hierarchical imports may have edge cases. Mitigation: test with stdlib modules (`std.collections`, `std.http`, etc.)

---

## Phase 2: Compile Tools (Week 1, Days 4-5)

### Goal
Enable the agent to validate its work through type-checking, compilation, and testing.

### Tasks

#### 2.1: Type-Check Integration
- [ ] Implement `check` tool
  - Call `compile_file()` with type-check-only mode (stop before codegen)
  - Parse diagnostics into structured JSON format
  - Include file path, span, line/column, severity, message
- [ ] Handle multi-module type-checking
  - Resolve imports and type-check entire project
  - Attribute errors to correct modules
- [ ] Return structured diagnostics format:
  ```json
  {
    "success": bool,
    "diagnostics": [{
      "severity": "error" | "warning",
      "message": string,
      "file": string,
      "span": { "start": int, "end": int },
      "line": int,
      "column": int
    }]
  }
  ```

**Success criteria:**
- `check` correctly type-checks multi-module projects
- Diagnostics are well-formatted and actionable
- 10+ tests for various error scenarios

#### 2.2: Compilation Tool
- [ ] Implement `compile` tool
  - Full pipeline: lex → parse → module resolve → typecheck → monomorphize → closures → codegen → link
  - Configurable output path
  - Return binary path on success
- [ ] Add compilation options support (if needed)
  - Optimization level
  - Debug symbols

**Success criteria:**
- `compile` produces working binaries
- Diagnostics on failure match `check` format
- 5+ tests for successful and failing compilations

#### 2.3: Run and Test Tools
- [ ] Implement `run` tool with execution safety
  - Compile to temp directory (outside project root)
  - Execute with timeout (default: 30s, max: 5 minutes)
  - **Safety measures:**
    - Set cwd to project root (confine file access to project)
    - Capture stdout/stderr with size limits (1MB each, truncate if exceeded)
    - Kill subprocess tree on timeout (use process group)
    - Clean up temp files on completion/error
  - Handle stdin input parameter
  - Return exit code, stdout, stderr, truncation warnings
- [ ] Implement `test` tool with same safety
  - Compile with test framework
  - Execute tests with `plutoc test`
  - Parse test output into structured format
  - Support test name filtering
  - Apply same safety measures as `run`
- [ ] Add process management
  - Timeout enforcement with subprocess tree kill
  - Graceful SIGTERM, then SIGKILL after 1s grace period
  - Resource cleanup in error paths (RAII wrappers)
  - Output size enforcement (stream with size tracking)

**Success criteria:**
- `run` executes programs and captures output correctly
- `test` parses test results into structured format
- 10+ tests including timeout scenarios

#### 2.4: Analyze Tool
- [ ] Implement `analyze` tool
  - Run front-end pipeline only (lex → parse → typecheck)
  - Compute derived data (error sets, resolved signatures)
  - Update in-memory module state with derived data
  - Don't write to disk (agent must call `save`)
- [ ] Return analysis stats
  - Functions analyzed
  - Error sets computed
  - Type errors found

**Success criteria:**
- `analyze` refreshes derived data without full compilation
- Subsequent queries return fresh derived data
- 5+ tests for analyze → query workflow

### Deliverables
- PR #2: Compile tools (check, compile, run, test, analyze)
- 30+ tests total (10 + 5 + 10 + 5)
- Documentation: Update MCP tools reference with compile tools

### Risk Mitigation
- **Compilation timeouts:** Long compilations may exceed MCP timeout. Mitigation: use separate timeout for compile tools, return progress updates if needed
- **Resource leaks:** Spawned processes may not clean up. Mitigation: use RAII wrappers, ensure cleanup in error paths

---

## Phase 3: Write Tools (Week 2, Days 1-2)

### Goal
Expose the complete SDK write API to Claude Code through MCP tools.

### Tasks

#### 3.1: Core Write Tools
- [ ] Implement path safety wrapper for all write tools
  - Normalize paths relative to project root
  - Reject paths outside project root (no `..` escape, no symlink escape)
  - Validate path points to existing module or valid new location
- [ ] Implement `add_declaration` tool
  - Parameters: path, source, position (before/after UUID or end)
  - **Validate path is within project root**
  - Delegate to `ModuleEditor::add_from_source()`
  - Return new UUID and diagnostics
- [ ] Implement `replace_declaration` tool
  - Parameters: id, source
  - Delegate to `ModuleEditor::replace_from_source()`
  - Preserve top-level UUID
  - Return diagnostics
- [ ] Implement `rename_declaration` tool
  - Parameters: id, new_name
  - Delegate to `ModuleEditor::rename()`
  - Return references_updated count
- [ ] Implement `delete_declaration` tool
  - Parameters: id
  - Delegate to `ModuleEditor::delete()`
  - Return dangling references list

**Success criteria:**
- All write tools correctly delegate to SDK
- UUIDs are preserved on replace/rename
- 16+ tests (4 per tool)

#### 3.2: Fine-Grained Write Tools
- [ ] Implement `add_method` tool
  - Parameters: class_id, source, position
  - Delegate to `ModuleEditor::add_method_from_source()`
- [ ] Implement `add_field` tool
  - Parameters: target_id, name, type, injected?
  - Delegate to `ModuleEditor::add_field()`
- [ ] Consider additional fine-grained tools:
  - `add_parameter` (for function signatures)
  - `add_variant` (for enums)
  - `update_signature` (change param/return types)

**Success criteria:**
- Fine-grained tools work for targeted edits
- Method/field additions preserve class structure
- 10+ tests for method/field operations

#### 3.3: State Management & Data Safety
- [ ] Add dirty tracking to server state
  - Mark modules dirty on any write operation
  - Track dirty set per session
  - **Track external modifications:** Detect if `.pluto` file changed outside MCP
  - Warn on save if external changes detected (potential conflict)
- [ ] Implement `save` tool with atomic writes
  - Parameters: path? (specific module or all dirty), backup?: bool (default: true)
  - **Atomic write protocol:**
    - Write to `.pluto.tmp` temporary file
    - fsync to ensure data on disk
    - Atomic rename `.pluto.tmp` → `.pluto`
  - **Optional backup:** Copy existing `.pluto` to `.pluto.backup` before save
  - **Validate path is within project root**
  - Serialize dirty modules to PLTO format
  - Clear dirty flag on success
  - Return list of saved paths
- [ ] Implement `reload` tool (discard dirty state, reload from disk)
  - Parameters: path? (specific module or all)
  - **Safety:** Error if dirty modules exist, unless `force: true` parameter
  - Clear module cache, reload from disk
  - Return list of reloaded paths and discarded change count
- [ ] Add `status` tool
  - Return: loaded modules, dirty modules, external modifications detected
  - Helps agent understand current state
- [ ] Add transaction semantics (optional, defer to Phase 6)
  - `begin_edit` / `commit_edit` / `rollback_edit`
  - In-memory snapshot for rollback
  - Defer: only if needed by Claude Code workflow

**Success criteria:**
- Dirty tracking prevents data loss
- `save` correctly flushes to disk
- 8+ tests for dirty tracking and save

### Deliverables
- PR #3: Write tools with state management
- 34+ tests total (16 + 10 + 8)
- Documentation: "MCP Write Operations" guide

### Risk Mitigation
- **Parse errors on write:** Agent may provide invalid source. Mitigation: return diagnostics, don't crash, preserve module state
- **UUID conflicts:** Rare but possible. Mitigation: UUID generation uses v4 random, collision probability negligible
- **Cross-module write consistency:** Writing to module A may invalidate references in module B. Mitigation: Phase 1 provides cross-module validation via `check` tool

---

## Phase 4: Format Tools (Week 2, Days 3-4)

### Goal
Enable bidirectional conversion between `.pluto` (binary, AI-native) and `.pt` (text, human-readable).

### Tasks

#### 4.1: Generate PT Tool with UUID Hints
- [ ] Implement `generate_pt` tool
  - Parameters: path? (specific module or all)
  - Use existing pretty printer
  - **Generate UUID hints:** Prepend `// @uuid: <uuid>` comment before each top-level declaration
  - Write `.pt` files alongside `.pluto` files
  - **Validate path is within project root**
  - Return list of generated files
- [ ] Update pretty printer to support UUID hints
  - Add optional `include_uuid_hints: bool` parameter to pretty-print functions
  - Format: `// @uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890` on line before declaration
- [ ] Add options:
  - Output directory (default: same as `.pluto`)
  - Overwrite behavior (fail, skip, overwrite)
  - include_uuids (default: true for MCP tool, false for manual pretty-print)

**Success criteria:**
- `generate_pt` creates human-readable text files
- Output is valid Pluto source (can be parsed back)
- 5+ tests for various modules

#### 4.2: CLI Sync Command with UUID Stability
- [ ] Implement `plutoc sync <file.pt>` command
  - Parse `.pt` text file into AST
  - Load corresponding `.pluto` binary (by naming convention)
  - Diff the two ASTs using UUID-hint matching (see Phase 0 decision)
  - **Matched declarations:** Preserve UUID from `.pluto`, take body from `.pt`
  - **New declarations in `.pt`:** Assign fresh UUIDs
  - **Deleted declarations:** Remove from `.pluto`
  - Write updated `.pluto` binary using atomic write protocol
- [ ] Implement AST diffing logic with UUID hints
  - **Primary matching:** Read `// @uuid: <uuid>` comments from `.pt`, use UUID to match declarations
  - **Fallback matching:** If no UUID hint, match by name + signature
  - **Signature matching for functions:** name + param types (handles body-only edits)
  - **Handle reordered declarations:** UUID/name matching prevents spurious delete+add
  - **Conflict detection:** Two .pt declarations with same UUID → error, manual resolution needed
- [ ] UUID merging strategy
  - Top-level UUID: Use UUID from comment if present, else from name match, else fresh UUID
  - Nested UUIDs (params, fields, methods): Match by name, preserve old UUID if found
  - Genuinely new nested items get fresh UUIDs
- [ ] Add sync validation
  - Warn if .pt contains declarations without UUID hints (unstable on rename)
  - Warn if .pluto was modified after .pt was generated (potential conflict)
  - Offer `--force` flag to override warnings

**Success criteria:**
- `sync` correctly merges human edits back into binary
- UUIDs are stable across sync cycles
- 15+ tests for various sync scenarios (renames, adds, deletes, reorders)

#### 4.3: MCP Sync Tool
- [ ] Implement `sync_pt` tool (MCP wrapper over CLI sync)
  - Parameters: pt_path
  - Call `plutoc sync` internally
  - Return sync stats: new/updated/deleted declaration counts
  - Return diagnostics on parse/type errors
- [ ] Implement `pretty_print` tool
  - Parameters: id? (single declaration) or path? (whole module)
  - Return source text without writing to disk
  - Use existing pretty printer

**Success criteria:**
- `sync_pt` and `pretty_print` work through MCP
- 5+ integration tests for MCP format tools

### Deliverables
- PR #4: Format tools (generate_pt, sync, pretty_print)
- 25+ tests total (5 + 15 + 5)
- Documentation: "Human ↔ AI Workflow" guide explaining .pt/.pluto roundtrip

### Risk Mitigation
- **Sync ambiguity:** Two declarations with same name but different signatures. Mitigation: treat as delete+add, log warning
- **Merge conflicts:** Human edits `.pt` while agent edits `.pluto`. Mitigation: last-write-wins for now, add conflict detection in future
- **Lossy pretty-print:** Comments and whitespace may not round-trip. Mitigation: document limitation, preserve formatting where possible

---

## Phase 5: Integration & Polish (Week 2, Day 5 - Week 3)

### Goal
End-to-end testing, documentation, examples, and production-readiness.

### Tasks

#### 5.1: End-to-End Testing
- [ ] Create comprehensive multi-module test project
  - 10+ modules with realistic dependencies
  - Tests, errors, DI, generics, traits
- [ ] Write E2E test scenarios:
  - "Add a new feature" (cross-module function call)
  - "Refactor a class" (rename + update callers)
  - "Fix a bug" (replace function body, run tests)
  - "Explore codebase" (list, find, inspect, xrefs)
- [ ] Test with real Claude Code instance
  - Configure Claude Code to use MCP server
  - Walk through E2E scenarios interactively
  - Document any UX issues or missing features

**Success criteria:**
- 5+ E2E scenarios pass end-to-end
- Claude Code can successfully complete realistic tasks
- 20+ integration tests for E2E workflows

#### 5.2: Performance & Reliability
- [ ] Benchmark MCP server with large project
  - 50+ modules, 500+ declarations
  - Measure: startup time, query latency, memory usage
  - Target: <2s startup, <100ms query latency, <500MB memory
- [ ] Add error handling improvements
  - Graceful degradation on parse errors
  - Better diagnostics for MCP tool errors
  - Logging for debugging (optional `--verbose` flag)
- [ ] Add health check and status tools
  - `status` tool: return server state (loaded modules, dirty modules)
  - `reload` tool: reload all modules from disk (discard dirty state)

**Success criteria:**
- Performance targets met on large project
- No crashes on malformed input
- 10+ stress tests

#### 5.3: Documentation
- [ ] Write "MCP Server User Guide"
  - Installation and setup
  - Tool reference (all 20+ tools)
  - Usage examples
  - Troubleshooting
- [ ] Write "Claude Code Integration Guide"
  - Configure `mcp-servers.json`
  - Example prompts for common tasks
  - Best practices for AI-driven development
- [ ] Update `docs/design/ai-native-pipeline.md`
  - Mark phases as complete
  - Update implementation status section
  - Add "Production Use" section
- [ ] Create video demo (optional)
  - Screen recording of Claude Code using MCP server
  - Show exploration, modification, compilation, testing

**Success criteria:**
- Documentation is complete and accurate
- New users can set up and use MCP server
- 3+ worked examples in docs

#### 5.4: Example Projects
- [ ] Create `examples/mcp-demo/` project
  - Multi-module Pluto project
  - README with Claude Code usage examples
  - Demonstrates: exploration, refactoring, testing
- [ ] Create `examples/ai-native-workflow/` guide
  - Step-by-step: start with binary `.pluto`, generate `.pt`, edit `.pt`, sync back
  - Show UUID stability across edits

**Success criteria:**
- Examples run without modification
- Examples demonstrate key MCP features

### Deliverables
- PR #5: E2E tests, performance improvements, documentation
- 30+ tests total (20 E2E + 10 stress)
- Documentation: User Guide, Integration Guide, updated RFC
- Example projects: mcp-demo, ai-native-workflow

### Risk Mitigation
- **Performance bottlenecks:** May discover issues only with large projects. Mitigation: profile early, optimize hot paths
- **Claude Code UX issues:** May need tool API adjustments based on real usage. Mitigation: iterate quickly, keep tool APIs flexible

---

## Phase 6: Advanced Features (Week 3+, Optional)

### Goal
Nice-to-have features that enhance the experience but aren't required for MVP.

### Tasks (Prioritized)

#### 6.1: Incremental Analysis
- [ ] Implement incremental type-checking
  - Only re-check modified modules and dependents
  - Cache type-check results
  - Invalidate cache on writes
- [ ] Add `check_module` tool (single-module type-check)

#### 6.2: Cross-Module Refactoring
- [ ] Implement `move_declaration` tool
  - Move declaration to different module
  - Update imports in all referencing modules
  - Preserve UUIDs
- [ ] Implement project-wide rename
  - Rename across all modules
  - Update all references automatically

#### 6.3: MCP Resources (Alternative to Tools)
- [ ] Expose modules as MCP resources
  - Subscribe to module changes
  - Real-time updates on file changes
- [ ] Expose diagnostics as resources
  - Subscribe to type-check results

#### 6.4: Binary Stability
- [ ] Evaluate bincode stability across Rust versions
- [ ] Consider migration to protobuf or flatbuffers
- [ ] Add schema versioning and migration

#### 6.5: Multi-Agent Support
- [ ] Add file locking for concurrent access
- [ ] Support multiple MCP server instances per project
- [ ] Add conflict resolution UI

### Deliverables
- Optional PRs based on priority and user feedback
- Documentation updates as features are added

---

## Success Metrics

### Completion Criteria
- [ ] All Phase 1-4 PRs merged to master
- [ ] 120+ tests passing (38 + 30 + 34 + 25 = 127 from phases 1-4)
- [ ] Phase 5 E2E tests pass
- [ ] Documentation complete
- [ ] Example projects working
- [ ] Claude Code can successfully use MCP server for real Pluto development

### Quality Gates
- Each PR must:
  - Pass all existing tests (no regressions)
  - Add tests for new functionality (min 80% coverage)
  - Include documentation updates
  - Be reviewed and approved
- Performance benchmarks must pass before Phase 5 completion
- E2E scenarios must pass before declaring MVP complete

---

## Phase 0: Decision Checkpoint ✅ COMPLETE

All critical decisions have been made and documented below.

### Decision 0.1: MCP Tool Naming Convention ✅

**DECIDED:** Align with RFC spec for consistency and clarity.

**Tool name mapping:**
- `inspect` → `get_declaration`
- `xrefs` → `callers_of` + `usages_of` (split by query type)
- `errors` → `error_set`
- `source` → `get_source`
- (new) → `find_declaration` (cross-project search)
- (new) → `call_graph`

**Implementation:** Rename existing tools in Phase 1, update all references.

**Rationale:** RFC names are more descriptive and self-documenting. Better API clarity.

---

### Decision 0.2: Execution Safety Model ✅

**DECIDED:** Trusted local model with basic safety measures.

**Safety measures included:**
- ✅ Working directory confined to project root
- ✅ Timeout enforcement (30s default, 5min max)
- ✅ Subprocess tree kill (SIGTERM → SIGKILL after 1s grace)
- ✅ Output size limits (1MB stdout, 1MB stderr, truncate with warning)
- ✅ Temp file cleanup (RAII wrappers)

**NOT included (deferred to Phase 6):**
- Network isolation
- Filesystem restrictions beyond cwd
- Chroot/container sandboxing
- Resource limits (CPU/memory caps)

**Implementation:** Phase 2 (compile tools)

**Rationale:** MCP server is for trusted local development. Same trust model as `cargo run`. Full sandboxing adds complexity with minimal benefit for this use case.

---

### Decision 0.3: UUID Stability Across .pt Rename ✅

**DECIDED:** Use UUID hint comments in generated `.pt` files.

**How it works:**

1. **Generate .pt with UUID hints:**
   ```pluto
   // @uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890
   pub fn cross_product(a: Vector, b: Vector) Vector { ... }
   ```

2. **Sync matches by UUID:**
   - Parse `@uuid` comments from `.pt`
   - Match declarations by UUID (primary)
   - Fall back to name+signature matching (if no UUID comment)
   - Preserve UUID on match, assign fresh UUID if genuinely new

3. **Renames preserve UUID:**
   - Human changes function name in `.pt`
   - UUID comment stays with declaration
   - Sync matches by UUID, preserves it, takes new name

**Implementation:** Phase 4 (format tools)

**Rationale:** Explicit, reliable, low implementation cost. Works with any text editor. Slight clutter in `.pt` files is acceptable trade-off for UUID stability.

---

## Phase 0 Decisions Summary

| Decision | Choice | Phase |
|----------|--------|-------|
| Tool naming | RFC spec names | Phase 1 |
| Execution safety | Trusted local + basic safety | Phase 2 |
| UUID stability | UUID hint comments | Phase 4 |

**Status:** ✅ All decisions locked in. Ready to proceed with Phase 1 implementation.

---

## Timeline

### Week 1: Foundation
- **Day 0:** Phase 0 decision checkpoint (resolve above 3 decisions)
- **Days 1-3:** Phase 1 (Project-Awareness)
- **Days 4-5:** Phase 2 (Compile Tools)

### Week 2: Write & Format
- **Days 1-2:** Phase 3 (Write Tools)
- **Days 3-4:** Phase 4 (Format Tools)
- **Day 5:** Phase 5 start (E2E testing)

### Week 3: Polish & Ship
- **Days 1-3:** Phase 5 (Integration, docs, examples)
- **Days 4-5:** Buffer for issues, Phase 6 if ahead of schedule

### Total: 2-3 weeks to MVP completion

---

## Dependencies

### External
- No external dependencies blocking this work
- Rust/Cargo toolchain (already set up)
- Claude Code (for testing, optional until Phase 5)

### Internal
- SDK write API ✅ (already complete)
- Pretty printer ✅ (already complete)
- PLTO binary format ✅ (already complete)
- Compiler front-end ✅ (already complete)

---

## Rollout Strategy

### Incremental Delivery
1. **PR #1 (Phase 1):** Makes read-only MCP server actually useful
2. **PR #2 (Phase 2):** Enables validation workflow
3. **PR #3 (Phase 3):** Enables modification workflow
4. **PR #4 (Phase 4):** Enables human-in-the-loop workflow
5. **PR #5 (Phase 5):** Production-ready release

Each PR delivers incremental value and can be used immediately.

### Testing Strategy
- Unit tests at each phase
- Integration tests with multi-module projects
- E2E tests in Phase 5
- Real usage testing with Claude Code in Phase 5

### Documentation Updates
- Update MEMORY.md after each phase with implementation notes
- Update ai-native-pipeline.md implementation status after each PR
- Create user-facing docs in Phase 5

---

## Open Questions

### Resolved in Phase 0
- [x] **MCP tool naming:** Align with RFC spec (get_declaration, callers_of, etc.) — **DECIDED: Use RFC names**
- [x] **Execution safety model:** Trusted local with basic safety (cwd confinement, timeout, output limits) — **DECIDED: See Phase 0.2**
- [x] **UUID stability across .pt rename:** Use UUID hint comments in generated .pt files — **DECIDED: See Phase 0.3**

### Deferred to Later Phases
- [ ] **Transaction model:** Explicit transactions or dirty tracking + save? **Defer to Phase 6** (dirty tracking sufficient for MVP)
- [ ] **Incremental type-checking:** Worth the complexity? **Defer to Phase 6** (full project type-check is fast enough for MVP)
- [ ] **Binary format stability:** Bincode vs protobuf? **Defer to Phase 6** (bincode sufficient for now, can migrate later)
- [ ] **MCP resources vs tools:** Expose modules as resources? **Defer to Phase 6** (tools are simpler, resources add complexity)
- [ ] **Full sandboxing:** Containers/chroot for untrusted code? **Defer to Phase 6** (trusted local model for MVP)

### New Questions from Plan Review
- [ ] **File watching:** Implement in Phase 1 or defer? **Decision:** Basic file watching in Phase 1 (detect external changes), advanced features in Phase 5/6
- [ ] **Conflict resolution UI:** How to handle .pt/.pluto conflicts? **Decision:** Warn + require --force for now, advanced UI in Phase 6
- [ ] **Backup retention:** How many .backup files to keep? **Decision:** Keep only latest .backup, manual cleanup

---

## Next Steps

1. **Review this plan** — Get feedback, adjust timeline/scope
2. **Create feature branch** — `ai-native-completion` (or use existing `mcp-server` branch)
3. **Start Phase 1, Task 1.1** — Project Index implementation
4. **Daily progress updates** — Update this doc with completion status
5. **PR after each phase** — Don't wait until all phases complete

Ready to start? 🚀
