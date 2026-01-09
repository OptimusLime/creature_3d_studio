# CrawlersChase Investigation

## Model Overview

**File:** `MarkovJunior/models/CrawlersChase.xml`
**Match:** 98.61% (50 cells different out of 3600)
**Grid:** 60x60x1

### Model Structure
```xml
<sequence values="BWRUGE">
  <one in="BB" out="UR" steps="12"/>          <!-- Place 12 crawler pairs -->
  <markov>
    <all>
      <rule in="URB" out="BUR"/>              <!-- Crawler moves right -->
      <rule in="UR/*B" out="BU/*R"/>          <!-- Crawler moves down -->
      <rule in="URW" out="BEG"/>              <!-- Crawler catches white -->
      <rule in="UR/*W" out="BE/*G"/>          <!-- Crawler catches white (down) -->
      <rule in="EG/BB" out="UR/RU"/>          <!-- Dead crawler spawns new one? -->
      <rule in="BB/BW" out="**/WB"/>          <!-- White moves down -->

      <field for="R" to="W" on="B" recompute="True" essential="True"/>
      <field for="W" from="R" on="B" recompute="True"/>
    </all>
    <one in="B" out="W"/>                     <!-- Place white creature -->
  </markov>
</sequence>
```

### Description
White creature tries to run away from a pack of crawlers. Crawlers can break.

## Initial Analysis

### Difference Pattern
- 50 cells different at 98.61% match
- Differences are scattered (X: 0-59, Y: 1-59)
- Both C# and Rust have B, W, R, U in diff positions
- Differences start at index 100 (early in execution)

### Key Components
1. **AllNode with Fields**: Uses field-guided selection
2. **Two fields**: 
   - `R to W on B` (essential) - crawlers path to white
   - `W from R on B` - white paths away from crawlers
3. **MarkovNode**: Contains AllNode + OneNode (place W)

## Hypotheses

### H1: Field computation difference
The model uses two fields with `recompute="True"`. Field computation involves:
- BFS/propagation through rules
- Potential values affecting match selection
- The `essential="True"` flag means failure if field can't compute

### H2: AllNode field-guided selection
AllNode uses fields for match scoring. Differences in:
- delta_pointwise calculation
- Match ordering/selection when scores tie

### H3: RNG divergence in AllNode
AllNode applies matches with probability `rule.p`. RNG differences could cause:
- Different matches being applied
- Different ordering effects

## Debug Plan

1. Create simplified test models:
   - L1: Just AllNode with basic rules (no fields)
   - L2: Add single field
   - L3: Add both fields
   - L4: Full markov structure

2. Compare step counts and match counts

3. Add debug output to field computation

## Commands

```bash
# Run C# version
cd MarkovJunior && dotnet run -- --model CrawlersChase --seed 42 --dump-json

# Run Rust version
MJ_MODELS=CrawlersChase MJ_SEED=42 cargo test -p studio_core verification::tests::batch_generate_outputs -- --ignored --nocapture

# Compare outputs
python3 scripts/compare_grids.py MarkovJunior/verification/CrawlersChase_seed42.json verification/rust/CrawlersChase_seed42.json
```

## Investigation Log

### Session Start
- Model: CrawlersChase
- Initial match: 98.61%
- Differences: 50 cells scattered across grid
