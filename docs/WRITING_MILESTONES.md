# Writing Milestones

This document describes how to write good milestones and construct build sequences that actually work.

---

## The Golden Rule

**Every milestone answers: "What can the user DO when this is done?"**

One sentence. "I can X." If you can't say it simply, the milestone is wrong.

---

## Principles

### 1. Each Milestone Delivers User-Visible Functionality

Milestones are not backends. Milestones are not systems. Milestones are functionality.

**Bad:** "In-memory material store"
**Good:** "I can pick from a list of materials"

**Bad:** "Lua engine integration"
**Good:** "I can edit a Lua file and see materials update without restarting"

**Bad:** "File watcher system"
**Good:** "I can save a script and see changes appear within 1 second"

The user doesn't care how it's stored. The user doesn't care if it's in-memory or on disk. The user cares what they can do.

### 2. End-to-End From Milestone 1

The first milestone produces a working app. It runs. It shows output. It responds to input.

Static implementations are fine. Hardcoded values are fine. But the whole pipeline works. You see something. You click something. Something changes.

**Why:** If you wait 5 milestones before anything works, you won't discover that milestone 1 was broken until milestone 5. That's disaster.

### 3. Coupled Functionality Ships Together

If feature A is useless without feature B, they are the same milestone.

**Example:** Lua scripting without hot reload is useless. Every edit requires an app restart. That defeats the purpose of external scripts. So "Lua materials" and "hot reload materials" are ONE milestone: "I can edit a Lua file and see materials update without restarting."

**Test:** Ask "Can I actually use this feature alone?" If no, it's not a milestone—it's half a milestone.

### 4. APIs Must Be Designed Before Implementation

You cannot implement something you haven't designed. If a milestone says "Lua generator," there must be:

- A designed `Generator` base class
- A designed lifecycle (`init`, `step`, `teardown`)
- Designed bindings (`ctx:set_voxel()`, `ctx:get_voxel()`)

API design is a deliverable. It can be part of a milestone or a prerequisite. But it cannot be assumed.

### 5. Name The APIs You're Implementing

Milestones reference specific traits, classes, and bindings:

**Bad:** "Generator defined in Lua file"
**Good:** "Implement `Generator` Lua base class with `init(ctx)`, `step(ctx)`, `teardown(ctx)` lifecycle. Expose `ctx:set_voxel(x, y, mat_id)` binding."

You don't need line-by-line detail. But you need to name the things being built.

### 6. Prove Integration Points Early

If two systems must integrate (Lua + Bevy, MCP + HTTP, file watcher + ECS), prove that integration works in a minimal form before building features on top.

Risky integrations fail in surprising ways. Don't discover that mlua doesn't play nice with Bevy's resource system in milestone 4.

---

## Milestone Template

```markdown
### M[N]: [Short Name]

**Functionality:** I can [what the user can do].

- [Visible outcome 1]
- [Visible outcome 2]
- Implements: [specific APIs, traits, classes, bindings]
- Proves: [integration points validated, if any]
```

---

## Example: Map Editor Build Sequence

### M1: Static End-to-End
**Functionality:** I can pick from 2 materials and see a checkerboard update.

- Window shows rendered 32x32 grid
- Material picker with stone, dirt
- Click material → checkerboard changes
- Static everything (no Lua, no files)

### M2: Lua Materials + Hot Reload
**Functionality:** I can edit a Lua file, save it, and see my materials update without restarting.

- Materials defined in `assets/materials.lua`
- Edit file → app updates within 1 second
- Implements: `MaterialStore` trait, `Material` Lua class, file watcher
- Proves: mlua + Bevy integration works

### M3: Lua Generator + Hot Reload
**Functionality:** I can edit a Lua generator script, save it, and see the terrain change without restarting.

- Generator defined in `assets/generator.lua`
- Edit file → terrain regenerates within 1 second
- Implements: `Generator` Lua base class (`init`/`step`/`teardown`), `ctx:set_voxel()` binding

### M4: External AI Access (MCP)
**Functionality:** An external AI can create materials and see the rendered output.

- AI calls `create_material` → appears in picker
- AI calls `get_output` → receives PNG
- Implements: MCP server, HTTP endpoints

---

## Audit Checklist

For each milestone, verify:

| Check | Question |
|-------|----------|
| Functionality | Can I state what the user can DO in one sentence? |
| End-to-End | Does M1 produce a working, visible app? |
| Coupled | Is every feature in this milestone usable on its own? If not, what's missing? |
| APIs Named | Did I name the specific traits/classes/bindings being built? |
| Integration | Are risky integrations proven before I build on them? |

If any check fails, revise the milestone.

---

## Common Mistakes

### Mistake: Backend-Indexed Milestones
"M1: Bevy shell. M2: In-memory materials. M3: Lua engine. M4: Generator. M5: Renderer."

**Problem:** 5 milestones before anything works. You don't know if M1 was broken until M5.

**Fix:** M1 is static end-to-end. Everything works. Then add complexity.

### Mistake: Splitting Coupled Features
"M3: Lua materials. M6: Hot reload materials."

**Problem:** Lua without hot reload is useless. You have to restart on every edit.

**Fix:** One milestone: "I can edit Lua and see materials update live."

### Mistake: Vague Functionality
"M2: Material system improvements"

**Problem:** What can I do? I have no idea.

**Fix:** "I can create materials with custom colors and see them in the picker."

### Mistake: Missing API Design
"M3: Generator from Lua"

**Problem:** What's the Generator API? What's the lifecycle? What bindings exist?

**Fix:** Either design the API in a prior milestone, or include "Design `Generator` base class" as an explicit deliverable.

---

## Summary

1. **Functionality first.** What can the user DO?
2. **End-to-end from M1.** Working app immediately.
3. **Coupled features ship together.** No half-usable milestones.
4. **Name your APIs.** Traits, classes, bindings—be specific.
5. **Prove integrations early.** Don't discover failures late.

If you can't explain the milestone in one sentence starting with "I can," it's not a milestone.
