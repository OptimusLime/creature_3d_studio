# SmartSAW Investigation

**Status:** RESOLVED
**Models:** SmartSAW.xml, CompleteSAW.xml, CompleteSAWSmart.xml

**Initial Match Rates:**
- SmartSAW: 45.43%
- CompleteSAW: 57.06%
- CompleteSAWSmart: 57.47%

**Final Match Rates (after all fixes):**
- SmartSAW: **100%**
- CompleteSAW: **100%**
- CompleteSAWSmart: **100%**

## Root Cause Summary

There were TWO distinct bugs affecting SAW models:

### Bug Set 1: Search Mode Not Wired Up (CompleteSAW, CompleteSAWSmart)

These models use `search="True"` attribute. Three sub-bugs:

1. **`run_search()` never called** - Search implementation existed but wasn't invoked
2. **Trajectory replay missing** - C# replays search results step-by-step
3. **Wrong RNG type** - Search used `StdRandom` instead of `DotNetRandom`

### Bug Set 2: Branch Node Execution Order (SmartSAW)

SmartSAW uses deeply nested markov/sequence structure WITHOUT search. The issue was in how branch children (MarkovNode, SequenceNode) delegate to their active child branches.

**Root Cause:** When a branch child completes (returns false), the parent was immediately falling through to try other children in the SAME Go() call. In C#, when `ip.current` returns to parent, the main loop increments counter BEFORE calling parent.Go() again.

**The Fix:** When an active branch child fails, return `true` from the parent to allow the counter increment to happen before the next attempt.

## Detailed Fix for Bug Set 2

### MarkovNode (node.rs)

**Before:**
```rust
if let Some(active_idx) = self.active_branch_child {
    if self.nodes[active_idx].go(ctx) { return true; }
    self.active_branch_child = None;
    // WRONG: Falls through to try children immediately
}
self.n = 0; // ...
```

**After:**
```rust
if let Some(active_idx) = self.active_branch_child {
    if self.nodes[active_idx].go(ctx) { return true; }
    self.active_branch_child = None;
    return true;  // FIX: Return to allow counter increment
}
self.n = 0; // ...
```

### SequenceNode (node.rs)

Same pattern - when active branch child fails, return `true` before advancing to next child:

```rust
if let Some(active_idx) = self.active_branch_child {
    if self.nodes[active_idx].go(ctx) { return true; }
    self.active_branch_child = None;
    self.n += 1;
    return true;  // FIX: Return to allow counter increment
}
```

## How We Found It

Used incremental layer testing per HOW_WE_WORK.md:

1. Created SmartSAWL1 through SmartSAWL7, each adding one layer of complexity
2. Found L1-L5 passed, L6 failed at 81.16%
3. Narrowed to the issue being two markov siblings in a parent markov
4. Further isolated to when an active branch child completes
5. Traced C# execution to find the counter increment timing difference

**Key test models created:**
- SmartSAWL1: Just union symbols
- SmartSAWL2: Union + initial all block
- SmartSAWL3-L5: Adding nested markov layers
- SmartSAWL6: Two markov siblings (first failure point)
- SmartSAWL6a-h: Various isolation tests

## Files Modified

### For Bug Set 1 (Search):
- `rule_node.rs` - Added `run_search()` call
- `one_node.rs` - Added trajectory replay
- `search.rs` - Changed to `DotNetRandom`

### For Bug Set 2 (Branch Execution):
- `node.rs` - Fixed MarkovNode and SequenceNode active_branch_child handling

## Verification Results

```
CompleteSAW:      57.06% -> 100% (FIXED - Bug Set 1)
CompleteSAWSmart: 57.47% -> 100% (FIXED - Bug Set 1)
SmartSAW:         45.43% -> 100% (FIXED - Bug Set 2)
```

## Commands

```bash
# Test all SAW variants
python3 scripts/batch_verify.py SmartSAW CompleteSAW CompleteSAWSmart --regenerate

# Test layer models
python3 scripts/batch_verify.py SmartSAWL1 SmartSAWL2 SmartSAWL3 SmartSAWL4 SmartSAWL5 SmartSAWL6 SmartSAWL7 --regenerate
```
