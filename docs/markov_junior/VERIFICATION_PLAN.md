# MarkovJunior Verification Plan - Phase 4

## Problem Statement

The MarkovJunior Rust port has 280 tests passing, but **visual verification is missing**. The examples in `cargo run` are NOT producing expected output. We suspect fundamental issues in the core algorithm (especially for 3D). 

Before debugging blindly, we need to **systematically verify** that our implementation matches the C# reference by **visual comparison**.

## Approach: Eyeball Verification First

1. **2D Models First**: Start with 2D models where we have clear reference images
2. **Side-by-Side Comparison**: Display our output next to C# reference image
3. **Identify Failures**: Note which models look wrong
4. **Then Automate**: Create pixel-comparison tests for specific failing cases

## Reference Data Available

From `MarkovJunior/` folder:
- **157 XML models** in `MarkovJunior/models/`
- **30 reference images** in `MarkovJunior/images/`
- **17 models with matching reference images** (for verification)

### 2D Models with Reference Images (Primary Targets)

| Model | Reference | Grid Size | Notes |
|-------|-----------|-----------|-------|
| Basic.xml | Basic.gif | 60x60 | Simplest: B→W random fill |
| Growth.xml | Growth.gif | 359x359 | WB→WW organic growth |
| MazeGrowth.xml | MazeGrowth.png | 359x359 | WBB→WAW maze corridors |
| MazeBacktracker.xml | MazeBacktracker.gif | 359x359 | Backtracking maze |
| DungeonGrowth.xml | DungeonGrowth.gif | 79x79 | Dungeon rooms |
| Flowers.xml | Flowers.gif | 60x60 | Pattern generation |
| Circuit.xml | Circuit.gif | 59x59 | Circuit-like patterns |
| River.xml | River.gif | 80x80 | River generation |
| Trail.xml | Trail.gif | 59x59 | Trail patterns |
| Wilson.xml | Wilson.gif | 59x59 | Wilson's algorithm maze |
| CompleteSAW.xml | CompleteSAW.gif | 19x19 | Self-avoiding walk |
| RegularSAW.xml | RegularSAW.gif | 39x39 | Regular SAW |
| LoopErasedWalk.xml | LoopErasedWalk.gif | - | Loop-erased walk |
| NystromDungeon.xml | NystromDungeon.gif | 39x39 | Nystrom dungeon |
| SokobanLevel1.xml | SokobanLevel1.gif | 14x9 | Sokoban puzzle |

### 3D Models with Reference Images

| Model | Reference | Grid Size | Notes |
|-------|-----------|-----------|-------|
| Apartemazements.xml | Apartemazements.gif | 8x8x8 | 3D apartments |
| StairsPath.xml | StairsPath.gif | 33x33x33 | 3D stairs |

---

## Phase 4.0: 2D Verification Infrastructure

**Outcome:** ImGui window with dropdown of 2D models, shows our render + reference image side-by-side.

### Tasks

1. **Copy reference images** to `assets/reference_images/mj/`
   - Copy all GIFs and PNGs from `MarkovJunior/images/`
   - Keep filenames consistent for matching

2. **Add `mj.list_models_with_refs()` Lua function**
   - Scans `MarkovJunior/models/*.xml`
   - Returns only models that have matching reference in `assets/reference_images/mj/`
   - Returns: `{name: string, xml_path: string, ref_path: string, is_3d: boolean}`

3. **Add `mj.load_model(xml_path)` improvements**
   - Load model directly from XML file path
   - Support full models.xml parameters (size, steps, etc.)

4. **Add `imgui.image_from_file(path)` Lua function**
   - Load image from disk
   - Create Bevy texture
   - Display in ImGui

5. **Create verification UI in main.lua**
   ```lua
   -- Verification window
   imgui.window("MJ Verification", function()
       -- Dropdown of models with refs
       local models = mj.list_models_with_refs()
       local selected = imgui.combo("Model", current_model, models)
       
       -- Generate button
       if imgui.button("Generate") then
           local model = mj.load_model(selected.xml_path)
           model:run({seed = current_seed, max_steps = 10000})
           local grid = model:grid()
           our_image = grid:render_to_image()  -- returns RGBA bytes
       end
       
       -- Side-by-side display
       imgui.columns(2)
       imgui.text("Our Output")
       imgui.image(our_image)
       imgui.next_column()
       imgui.text("Reference")
       imgui.image_from_file(selected.ref_path)
   end)
   ```

### Verification Criteria

1. Run `cargo run`
2. Open "MJ Verification" window
3. Select "MazeGrowth" from dropdown
4. Click Generate
5. See our render on LEFT, reference image on RIGHT
6. Visually compare - patterns should look similar (not identical due to random seeds)

---

## Phase 4.1: 2D Model Testing

**Outcome:** Systematically test each 2D model, document which pass/fail visual inspection.

### Tasks

1. **For each 2D model with reference:**
   - Generate with seed 0
   - Compare visually to reference
   - Document result: PASS / FAIL / PARTIAL
   - If FAIL: note what's wrong (colors, patterns, empty, etc.)

2. **Create `docs/markov_junior/VERIFICATION_RESULTS.md`**
   - Table of all tested models
   - Pass/fail status
   - Notes on failures

### Expected Issues to Find

- Color mapping problems (wrong palette)
- Algorithm bugs (patterns don't match)
- Missing node types (certain XML features not implemented)
- Performance issues (too slow to complete)

---

## Phase 4.2: Targeted Bug Fixes

**Outcome:** Fix the specific issues found in Phase 4.1.

### Tasks

For each failing model:
1. Identify root cause
2. Create minimal test case
3. Fix the bug
4. Verify fix with visual comparison
5. Add automated test if possible

---

## Phase 4.3: 3D Verification

**Outcome:** Same verification flow for 3D models.

### Tasks

1. Add 3D models to verification dropdown
2. Render 3D isometric for comparison
3. Test each 3D model
4. Document results

---

## Phase 4.4: Automated Regression Tests

**Outcome:** Automated tests that catch regressions.

### Tasks

1. For each verified-working model:
   - Generate with fixed seed
   - Save reference PNG (our known-good output)
   - Create test that compares future runs to this reference

2. Add to CI pipeline

---

## Directory Structure

```
assets/
  reference_images/
    mj/
      Basic.gif
      Growth.gif
      MazeGrowth.png
      ... (copied from MarkovJunior/images/)
  scripts/
    ui/
      main.lua                    # Updated with verification UI
      mj_verification.lua         # Optional: separate verification script

docs/
  markov_junior/
    VERIFICATION_PLAN.md          # This document
    VERIFICATION_RESULTS.md       # Results table (created in 4.1)
```

---

## Success Criteria

Phase 4.0 is complete when:
- [ ] Reference images copied to `assets/reference_images/mj/`
- [ ] Dropdown shows all 2D models with references
- [ ] Can select model and generate
- [ ] Our output displays in ImGui
- [ ] Reference image displays next to our output
- [ ] Visual comparison is easy and immediate

Phase 4.1 is complete when:
- [ ] All 15+ 2D models tested
- [ ] Results documented in VERIFICATION_RESULTS.md
- [ ] Pass/fail status for each model

---

## Commands

```bash
# Run main app with verification UI
cargo run

# Run tests (should still be 280)
cargo test -p studio_core markov_junior
```

---

## Notes

- **GIF files**: Many references are GIFs (animated). For comparison, we'll use the final frame or a representative frame.
- **Random seeds**: Output won't match pixel-perfect due to random seeds. We're looking for **structural similarity** - does our maze look like a maze? Does our growth look organic?
- **Color matching**: Must use same palette.xml colors as C#
- **Grid sizes**: Use sizes from models.xml for proper comparison
