# RegularPath Debugging Investigation

## Problem Statement
RegularPath model matches 98.13% (117 cells differ out of 6241).
We need to find the root cause of divergence between C# and Rust implementations.

## Model Structure
```xml
<sequence values="BAWRGY" origin="True">
  <all in="A*B" out="**A"/>           <!-- Step 1: Grow A from origin -->
  <one in="A" out="R" steps="1"/>     <!-- Step 2: Place R (start point) -->
  <one in="A" out="Y" steps="10"/>    <!-- Step 3: Place 10 Y cells (targets) -->
  <all in="A" out="B"/>               <!-- Step 4: Convert remaining A to B -->
  <markov>                            <!-- Step 5: Main loop -->
    <one in="RBB" out="WWR">          <!-- Path finding with observations -->
      <observe value="G" from="B" to="R"/>
      <observe value="B" to="BW"/>
      <observe value="R" to="W"/>
    </one>
    <prl in="W" out="A"/>             <!-- Convert W back to A -->
    <one in="Y" out="G"/>             <!-- Convert Y to G (goal) -->
  </markov>
</sequence>
```

## Comparison Results
```
First differences:
  (15,7,0): C#=0(B) Rust=4(G)
  (31,27,0): C#=0(B) Rust=4(G)
  (75,37,0): C#=1(A) Rust=0(B)
  ... (117 total differences)
```

## Hypotheses

### H1: Observation computation timing
The `<one in="Y" out="G"/>` creates G cells AFTER observations might be computed.
New G cells may not be processed by `compute_future_set_present`.

**Test:** Run model with only 1 Y cell (steps="1") to simplify.

### H2: Inline rule vs explicit rule difference
RegularPath uses `<one in="RBB" out="WWR">` (inline rule).
Working models like DiagonalPath use `<one><rule .../></one>` (explicit rules).

**Test:** Create variant with explicit rule syntax.

### H3: Match selection tie-breaking
With temperature=0, key = -h + 0.001*u. When multiple matches have same delta,
random tiebreaker determines winner. Order of match iteration affects which
random value each match gets.

**Test:** Add debug output to print match list and selected match.

### H4: Backward potential computation difference
Observations create backward potentials via `compute_backward_potentials`.
The BFS propagation through rules may differ.

**Test:** Dump potentials after computation, compare C# vs Rust.

## Simplification Plan

### Level 1: Minimal observation model
```xml
<sequence values="BRG" size="20 20" origin="True">
  <one in="B" out="R" steps="1"/>
  <one in="B" out="G" steps="1"/>
  <one in="RB" out="WR">
    <observe value="G" from="B" to="R"/>
    <observe value="B" to="BW"/>
    <observe value="R" to="W"/>
  </one>
</sequence>
```
Simplest path: R to G with observations.

### Level 2: Add multiple targets
Same as Level 1 but with `steps="3"` for G placement.

### Level 3: Add the prl node
Add `<prl in="W" out="A"/>` to test interaction.

### Level 4: Full markov loop
Add the markov wrapper and Y->G conversion.

## What We Know (Facts)
- [ ] Steps 1-4 produce identical output (need to verify)
- [ ] Divergence starts at step 5 (markov loop)
- [ ] First diff at index 568 = position (15,7)
- [ ] C# has B where Rust has G at (15,7) and (31,27)

## What We Need to Verify
- [ ] Are observations being loaded correctly for inline rules?
- [ ] Is backward potential computation identical?
- [ ] Is match ordering identical before selection?
- [ ] At what exact step does divergence begin?

## Simplification Test Results

| Level | Description | Result |
|-------|-------------|--------|
| L1 | R, G, path with observations | PASS |
| L2 | R, 3x G, path with observations | PASS |
| L3 | L2 + markov + prl W->A | PASS |
| L4 | L3 but Y->G inside markov | **FAIL (9 diff)** |

### Level 4 Failure Analysis
```
TestObsL4: 97.75% (9 cells differ)

Differences:
  (9,2,0): C#=4(A) Rust=1(R)   <- C# completed path, Rust still has R
  (10-15,2,0): C#=A, Rust=B   <- C# traversed, Rust didn't
  (16,2,0): C#=1(R) Rust=2(G) <- C# reached this, Rust has G
  (7,10,0): C#=0(B) Rust=2(G) <- C# converted G->B, Rust didn't
```

**Key Insight:** The Y->G conversion inside markov creates G cells AFTER
observations were computed. These late G cells:
- Don't get `state[i] = obs.from` treatment (G->B conversion)
- Don't have proper `future[i]` constraints

### Debug Output Comparison
```
C#:   "DEBUG: observed value 2 not present" x2 (called twice)
Rust: "DEBUG: observed value 2 not present" x1 (called once)
```

**CRITICAL FINDING:** C# calls `ComputeFutureSetPresent` TWICE before succeeding,
but Rust only calls it ONCE. This means there's a difference in retry logic!

Possible causes:
1. Rust's `future_computed` flag is set incorrectly
2. Rust's node reset doesn't clear `future_computed` properly
3. Different markov loop behavior

## Refined Hypothesis

**H5: Late G cells not processed by observations**
When `<one in="Y" out="G"/>` runs AFTER `future_computed=true`,
the new G cells are not transformed (G->B) by observations.

**Test:** Add debug output to trace when G cells are created vs when
observations are computed.

## Next Actions
1. Add debug output to C# to see observation timing
2. Add same debug output to Rust
3. Compare when future_computed becomes true relative to G cell creation
4. Fix Rust to handle late observed values

## Debug Instrumentation Needed
- C#: Add step counter output, dump grid state at key points
- Rust: Same instrumentation for comparison
- Compare at: after observations computed, after each markov iteration
