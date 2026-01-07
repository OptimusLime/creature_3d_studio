# SmartSAW Investigation

**Status:** PARTIALLY RESOLVED
**Models:** SmartSAW.xml, CompleteSAW.xml, CompleteSAWSmart.xml

**Initial Match Rates:**
- SmartSAW: 45.43%
- CompleteSAW: 57.06%
- CompleteSAWSmart: 57.47%

**Final Match Rates (after fix):**
- SmartSAW: 45.43% (unchanged - different issue, see below)
- CompleteSAW: **100%**
- CompleteSAWSmart: **100%**

## Root Cause: Search Mode Not Wired Up

### Bug 1: `run_search()` never called

**Location:** `rule_node.rs:compute_matches()`

**Problem:** The `search` flag was set when parsing XML, but the actual A* search was never executed. The search implementation existed in `search.rs` but was only called in unit tests.

**C# Reference:** `RuleNode.cs:137-142`
```csharp
if (search)
{
    trajectory = null;
    int TRIES = limit < 0 ? 1 : 20;
    for (int k = 0; k < TRIES && trajectory == null; k++) 
        trajectory = Search.Run(grid.state, future, rules, grid.MX, grid.MY, grid.MZ, 
                               grid.C, this is AllNode, limit, depthCoefficient, ip.random.Next());
    if (trajectory == null) Console.WriteLine("SEARCH RETURNED NULL");
}
```

**Fix:** Added call to `run_search()` in `compute_matches()` when `self.search == true` and `self.future_computed == true`:
```rust
if self.search {
    self.trajectory = None;
    let tries = if self.limit < 0 { 1 } else { 20 };
    for _ in 0..tries {
        if self.trajectory.is_some() { break; }
        let seed = ctx.random.next_int();
        self.trajectory = run_search(
            &ctx.grid.state, future, &self.rules,
            ctx.grid.mx, ctx.grid.my, ctx.grid.mz,
            ctx.grid.c as usize, is_all,
            self.limit, self.depth_coefficient, seed,
        );
    }
}
```

### Bug 2: Trajectory replay missing in OneNode

**Location:** `one_node.rs:go()`

**Problem:** When a trajectory is computed by search, C# replays it step-by-step by copying states. The Rust implementation went directly to random matching without checking for a trajectory.

**C# Reference:** `OneNode.cs:56-62`
```csharp
if (trajectory != null)
{
    if (counter >= trajectory.Length) return false;
    Array.Copy(trajectory[counter], grid.state, grid.state.Length);
    counter++;
    return true;
}
```

**Fix:** Added trajectory replay before random matching:
```rust
if let Some(ref trajectory) = self.data.trajectory {
    if self.data.counter >= trajectory.len() {
        return false;
    }
    ctx.grid.state.copy_from_slice(&trajectory[self.data.counter]);
    self.data.counter += 1;
    return true;
}
```

### Bug 3: Wrong RNG type for search seed

**Location:** `search.rs` and `rule_node.rs`

**Problem:** Search was using `StdRandom` with a u64 seed, but C# uses `new Random(seed)` with an i32 seed from `ip.random.Next()`.

**Fix:** 
1. Changed `search.rs` to use `DotNetRandom::from_seed(seed)` instead of `StdRandom`
2. Changed seed parameter type from `u64` to `i32`
3. Changed call site to use `ctx.random.next_int()` instead of `next_u64()`

## Files Modified

### `crates/studio_core/src/markov_junior/rule_node.rs`
- Added import for `run_search`
- Added `is_all: bool` parameter to `compute_matches()`
- Added search execution logic when `self.search == true`

### `crates/studio_core/src/markov_junior/one_node.rs`
- Added trajectory replay logic before random matching
- Updated `compute_matches()` call to pass `is_all=false`

### `crates/studio_core/src/markov_junior/all_node.rs`
- Updated `compute_matches()` call to pass `is_all=true`

### `crates/studio_core/src/markov_junior/search.rs`
- Changed from `StdRandom` to `DotNetRandom` for RNG
- Changed seed parameter type from `u64` to `i32`

## Remaining Issue: SmartSAW

SmartSAW is still at 45.43% match rate. This is a **different issue** because SmartSAW does NOT use `search="True"`. Looking at its structure:

```xml
<sequence values="BRDYGWEUN" origin="True">
  <union symbol="?" values="BD"/>
  <union symbol="_" values="BN"/>
  <!-- ... nested markov/sequence structure ... -->
</sequence>
```

SmartSAW uses:
- Union symbols (`?` = BD, `_` = BN) 
- Deeply nested markov/sequence control flow
- No search attribute

This requires a separate investigation focused on:
1. Union symbol handling
2. Nested control flow execution order

## Verification Results

```
CompleteSAW:      57.06% -> 100% (FIXED)
CompleteSAWSmart: 57.47% -> 100% (FIXED)
SmartSAW:         45.43% -> 45.43% (separate issue)
```

## Commands

```bash
# Test CompleteSAW
cd MarkovJunior && dotnet run -- --model CompleteSAW --seed 42 --dump-json
MJ_MODELS=CompleteSAW MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture
python3 scripts/compare_grids.py MarkovJunior/verification/CompleteSAW_seed42.json verification/rust/CompleteSAW_seed42.json

# Test CompleteSAWSmart  
cd MarkovJunior && dotnet run -- --model CompleteSAWSmart --seed 42 --dump-json
MJ_MODELS=CompleteSAWSmart MJ_SEED=42 cargo test -p studio_core batch_generate_outputs -- --ignored --nocapture
python3 scripts/compare_grids.py MarkovJunior/verification/CompleteSAWSmart_seed42.json verification/rust/CompleteSAWSmart_seed42.json
```
