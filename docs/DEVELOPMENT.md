# Development Methodology

## Core Principles

1. **Incremental MVP** - Build v0 → v1 → v2, never big-bang
2. **Facade-First** - Stable APIs that hide implementation churn
3. **Verification-Driven** - Every phase ends with measurable pass/fail criteria
4. **Early End-to-End** - Get the full pipeline working thin, then thicken

## Project Structure

```
creature_3d_studio/
├── src/
│   ├── main.rs              # Binary entry point
│   └── lib.rs               # Re-exports from crates
├── crates/
│   ├── studio_core/         # Main game/editor library (ECS, scene)
│   ├── studio_physics/      # Rapier integration, physics scene
│   └── studio_scripting/    # bevy_mod_scripting + ImGui facade
├── assets/
│   └── scripts/
│       └── ui/              # Hot-reloadable Lua UI scripts
├── docs/
│   ├── DEVELOPMENT.md       # This file
│   ├── ARCHITECTURE.md      # System design
│   └── versions/            # Feature plans per version
│       └── v0.1/
│           └── plan.md
└── .github/
    └── PULL_REQUEST_TEMPLATE.md
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `bevy` | Game engine, ECS |
| `bevy_mod_imgui` | ImGui rendering |
| `bevy_mod_scripting` | Lua54 scripting with Bevy integration |
| `rapier3d` | Physics simulation |

## Feature Development Workflow

### 1. Branch Creation
```bash
git checkout -b feature/<name>
```

### 2. Planning Phase (BEFORE any code)

Create `docs/versions/vX.Y/plan.md` with:

```markdown
# vX.Y - <Feature Name>

## Goal
One sentence describing the deliverable.

## Phases

### Phase 1: <Name>
**Tasks:**
- [ ] Task 1 (specific, measurable)
- [ ] Task 2

**Verification:**
- `cargo test --package <pkg>` passes
- Running `cargo run` shows <specific observable behavior>

### Phase 2: <Name>
...
```

### 3. Execution
- Work phase-by-phase
- Complete verification before moving to next phase
- Update todo list as you progress

### 4. Pull Request
- Open as Draft/WIP early
- Use PR template
- Include screenshots for UI work
- Squash-merge when approved

## Task Design: SMART Criteria

Every task must be:
- **S**pecific - "Add `imgui.button` facade" not "work on imgui"
- **M**easurable - "Lua script can call `imgui.button` and see click response"
- **A**chievable - Can complete in one focused session
- **R**elevant - Directly serves the phase goal
- **T**ime-bound - Implicit via phase structure

## Verification Standards

**Bad:** "It runs"  
**Good:** "`cargo run` opens window with title 'Creature Builder' containing text 'Lua-driven ImGui is live.'"

**Bad:** "Tests pass"  
**Good:** "`cargo test -p creature_imgui_lua` exits 0 with 3/3 tests passing"

Verification should be:
1. Copy-pasteable commands
2. Observable outcomes (screenshot, stdout, exit code)
3. Written BEFORE implementation begins

## Version Numbering

- `v0.x` - Pre-release, API unstable
- `v1.0` - First stable release
- Patch versions for bugfixes within a feature set

## Documentation Updates

Update docs as part of the feature, not after:
- `ARCHITECTURE.md` when adding new systems
- `README.md` when changing how to run/build
- Version plan marked complete when merged
