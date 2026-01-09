# River.xml Debugging Plan

## Problem Statement

River.xml produces output that doesn't match the reference. The output shows two colors duking it out (W and R growth) but never transitions to the river-carving phase.

## River.xml Structure

```xml
<sequence values="BWRUGE">
  <one in="B" out="W" steps="1"/>           <!-- Phase 1: Place single W -->
  <one in="B" out="R" steps="1"/>           <!-- Phase 2: Place single R -->
  <one>                                      <!-- Phase 3: W and R grow until they meet -->
    <rule in="RB" out="RR"/>
    <rule in="WB" out="WW"/>
  </one>
  <all in="RW" out="UU"/>                   <!-- Phase 4: Convert RW borders to U (river) -->
  <all>                                      <!-- Phase 5: Remove W and R -->
    <rule in="W" out="B"/>
    <rule in="R" out="B"/>
  </all>
  <all in="UB" out="UU" steps="1"/>         <!-- Phase 6: Expand river once -->
  <all in="BU/UB" out="U*/**"/>             <!-- Phase 7: River corner fill -->
  <all in="UB" out="*G"/>                   <!-- Phase 8: Mark river banks as G -->
  <one in="B" out="E" steps="13"/>          <!-- Phase 9: Plant trees (13 total) -->
  <one>                                      <!-- Phase 10: Trees and banks grow -->
    <rule in="EB" out="*E"/>
    <rule in="GB" out="*G"/>
  </one>
</sequence>
```

## Expected Behavior

1. **Phase 1-2**: Place one W and one R somewhere on the grid (seeds)
2. **Phase 3**: W and R expand until they fill the grid and meet
3. **Phase 4**: Where R touches W, convert to U (river starts)
4. **Phase 5**: Clear W and R, leaving only U and B
5. **Phase 6-8**: River expands and banks form
6. **Phase 9-10**: Trees grow

## Hypothesis: Sequence Transition Bug

The most likely issue is that our `SequenceNode` is:
1. Not correctly detecting when a child node has "completed"
2. The `<one>` node with `steps="1"` might not be returning `false` after 1 application
3. Or the growth `<one>` might never "complete" (always has matches)

### Key C# Behavior

In C#, `<one>` returns `false` when:
- No matches exist, OR
- `steps` limit reached AND `counter >= steps`

After `<one>` returns `false`, `SequenceNode` advances to next child.

## Test Plan

### Step 1: Incremental Screenshot Test

Create a test that saves screenshots at each step to visualize:
- When phases transition
- What the grid looks like at each phase

### Step 2: Add Debugging to SequenceNode

Log when children start/end and why.

### Step 3: Verify Steps Attribute

Ensure `steps="1"` correctly limits to 1 application.

## Verification Criteria

River.xml is fixed when:
1. Output shows a winding river pattern (U character)
2. Green banks (G) surround the river
3. Trees (E) scattered in remaining area
4. NOT just two colors competing

## Files to Modify

- `crates/studio_core/src/markov_junior/render.rs` - Add incremental screenshot test
- `crates/studio_core/src/markov_junior/node.rs` - Debug SequenceNode if needed
- `crates/studio_core/src/markov_junior/one_node.rs` - Verify steps handling
