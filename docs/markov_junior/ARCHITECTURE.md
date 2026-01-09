# MarkovJunior Architecture Analysis

This document provides an extremely detailed analysis of the MarkovJunior system for porting to Rust + Lua.

## Overview

MarkovJunior is a **probabilistic programming language** where:
- Programs are combinations of **rewrite rules**
- Inference is performed via **constraint propagation**
- Named after Andrey Markov's "Markov algorithms"

The system generalizes 1D Markov algorithms to N-dimensional grids (2D/3D).

---

## Core Data Structures

### 1. Grid (`Grid.cs` - 139 lines)

The Grid represents the world state - a 2D or 3D array of byte values.

```
Grid {
    state: byte[]           // Flat array of cell values [MX * MY * MZ]
    mask: bool[]            // Used by AllNode for conflict detection
    MX, MY, MZ: int         // Dimensions (MZ=1 for 2D)
    
    C: byte                 // Number of distinct values (colors)
    characters: char[]      // Character symbol for each value index
    values: Dict<char, byte>  // Symbol -> value index
    waves: Dict<char, int>  // Symbol -> wave (bitmask of allowed values)
                            // '*' = wildcard = (1 << C) - 1
    
    folder: string          // Optional folder for rule files
    transparent: int        // Wave of transparent values
    statebuffer: byte[]     // Temporary buffer for state operations
}
```

**Key Methods:**
- `Wave(string values) -> int`: Converts string like "RBW" to bitmask
- `Matches(Rule, x, y, z) -> bool`: Checks if rule input matches at position
- `Clear()`: Sets all state to 0

**Indexing Convention:**
```
index = x + y * MX + z * MX * MY
```

### 2. Rule (`Rule.cs` - 283 lines)

A Rule defines a pattern transformation: `input -> output`

```
Rule {
    // Input pattern dimensions
    IMX, IMY, IMZ: int
    
    // Output pattern dimensions  
    OMX, OMY, OMZ: int
    
    // Input pattern as wave array (bitmasks)
    // Wave allows multiple values: "RB" means Red OR Blue
    input: int[]            // [IMX * IMY * IMZ]
    
    // Output pattern as byte array
    // 0xff = wildcard (don't change)
    output: byte[]          // [OMX * OMY * OMZ]
    
    // Binary input for fast matching
    // 0xff = wildcard, else = single value index
    binput: byte[]
    
    // Probability of applying rule
    p: double               // Default 1.0
    
    // Pre-computed shifts for fast pattern matching
    // ishifts[c] = positions where color c appears in input
    ishifts: (int,int,int)[][]
    
    // oshifts[c] = positions where color c appears in output
    oshifts: (int,int,int)[][]
    
    original: bool          // True if not a symmetry variant
}
```

**Key Methods:**
- `ZRotated() -> Rule`: Rotate 90 degrees around Z axis
- `YRotated() -> Rule`: Rotate 90 degrees around Y axis
- `Reflected() -> Rule`: Mirror along X axis
- `Symmetries(bool[], bool d2) -> IEnumerable<Rule>`: Generate all symmetry variants

**Rule String Parsing:**
```
"RBB/WWW"  = 2 rows (Y), 3 columns (X)
"RBB WWW"  = 2 layers (Z) of 1x3
Slashes (/) = Y separator
Spaces ( ) = Z separator
```

### 3. Node Hierarchy

```
Node (abstract)
├── Branch (abstract) - contains child nodes
│   ├── SequenceNode - execute children in order
│   ├── MarkovNode - execute first matching child repeatedly
│   ├── MapNode - scale/transform grid
│   └── WFCNode (abstract) - Wave Function Collapse
│       ├── TileNode - tile-based WFC
│       └── OverlapNode - overlapping model WFC
│
└── RuleNode (abstract) - contains rules
    ├── OneNode - apply ONE random match
    ├── AllNode - apply ALL non-conflicting matches
    └── ParallelNode - apply ALL matches (may conflict)

ConvolutionNode - cellular automata (not RuleNode)
ConvChainNode - ConvChain algorithm (not RuleNode)
PathNode - Dijkstra path finding (not RuleNode)
```

---

## Node Types in Detail

### Branch Nodes

#### SequenceNode
Executes children in order. When child returns false, moves to next. When all children exhausted, returns false and resets.

```rust
fn go(&mut self) -> bool {
    while self.n < self.nodes.len() {
        if self.nodes[self.n].go() {
            return true;
        }
        self.n += 1;
    }
    self.reset();
    false
}
```

#### MarkovNode
Same as SequenceNode but resets `n = 0` before each Go(). This creates a loop that always tries the first matching child.

```rust
fn go(&mut self) -> bool {
    self.n = 0;  // Always start from first child
    // Then same as SequenceNode
}
```

This is the KEY difference - Markov nodes create loops by always trying from the beginning.

### RuleNode Base (`RuleNode.cs` - 219 lines)

Contains shared logic for rule-based nodes:

```
RuleNode {
    rules: Rule[]
    counter: int            // Execution count
    steps: int              // Max steps (0 = unlimited)
    
    // Match tracking
    matches: List<(r, x, y, z)>  // Rule index + position
    matchCount: int
    lastMatchedTurn: int
    matchMask: bool[][]     // [rule_idx][grid_pos] = already matched?
    
    // Field/potential system (for inference)
    potentials: int[][]     // [color][grid_pos] = distance
    fields: Field[]
    
    // Observation/inference
    observations: Observation[]
    temperature: double
    search: bool
    futureComputed: bool
    future: int[]           // Target state waves
    trajectory: byte[][]    // Pre-computed path from search
    
    // Search parameters
    limit: int
    depthCoefficient: double
    
    last: bool[]            // Which rules fired last turn
}
```

**Pattern Matching Algorithm (Go method):**

1. If first call (`lastMatchedTurn < 0`): Full grid scan using Boyer-Moore style optimization
2. If subsequent call: Only check cells that changed since last turn

```rust
// Incremental matching - only check changed cells
for (x, y, z) in changes_since_last_turn {
    let value = grid.state[index];
    for (r, rule) in rules.enumerate() {
        // Use ishifts to find which input positions could be affected
        for (dx, dy, dz) in rule.ishifts[value] {
            let sx = x - dx;
            let sy = y - dy;
            let sz = z - dz;
            // Check bounds and if rule matches
            if in_bounds && !mask[r][si] && grid.matches(rule, sx, sy, sz) {
                add_match(r, sx, sy, sz);
            }
        }
    }
}
```

### OneNode (`OneNode.cs` - 140 lines)

Picks ONE random matching rule and applies it.

**Selection Methods:**
1. **Without potentials**: Uniform random selection from valid matches
2. **With potentials**: Boltzmann distribution based on heuristic

```rust
fn random_match(&mut self) -> Option<(r, x, y, z)> {
    if self.potentials.is_some() {
        // Heuristic-guided selection
        let mut best_key = -1000.0;
        let mut best = None;
        
        for match in matches {
            let heuristic = Field::delta_pointwise(...);
            if heuristic.is_none() { continue; }
            
            let h = heuristic.unwrap();
            let u = random();
            let key = if temperature > 0 {
                u.powf((h - first_h) / temperature)
            } else {
                -h + 0.001 * u  // Greedy with tie-breaking
            };
            
            if key > best_key {
                best_key = key;
                best = Some(match);
            }
        }
        best
    } else {
        // Simple random selection
        while match_count > 0 {
            let idx = random(match_count);
            let m = matches[idx];
            if grid.matches(rules[m.r], m.x, m.y, m.z) {
                return Some(m);
            }
            // Remove invalid match
            swap_remove(idx);
        }
        None
    }
}
```

### AllNode (`AllNode.cs` - 108 lines)

Applies ALL non-conflicting matches in one step.

**Conflict Resolution:**
Uses `grid.mask` to track which cells have been modified this turn. Skips rules that would write to already-modified cells.

```rust
fn fit(&mut self, r: usize, x: i32, y: i32, z: i32, newstate: &mut [bool]) {
    let rule = &self.rules[r];
    
    // Check for conflicts
    for dz in 0..rule.OMZ {
        for dy in 0..rule.OMY {
            for dx in 0..rule.OMX {
                let value = rule.output[dx + dy * OMX + dz * OMX * OMY];
                if value != 0xff && newstate[target_index] {
                    return;  // Conflict - skip this match
                }
            }
        }
    }
    
    // No conflict - apply rule
    self.last[r] = true;
    for dz in 0..rule.OMZ {
        for dy in 0..rule.OMY {
            for dx in 0..rule.OMX {
                let newvalue = rule.output[...];
                if newvalue != 0xff {
                    newstate[i] = true;
                    grid.state[i] = newvalue;
                    changes.push((sx, sy, sz));
                }
            }
        }
    }
}
```

### ParallelNode (`ParallelNode.cs` - 50 lines)

Simpler than AllNode - applies ALL matches independently with NO conflict checking. Results are non-deterministic when rules overlap.

Uses a double-buffered approach:
1. Collect all changes to `newstate`
2. Copy `newstate` back to `grid.state`

---

## Inference System

### Observation (`Observation.cs` - 184 lines)

Observations define future constraints - what the grid SHOULD become.

```
Observation {
    from: byte      // Treat observed cells as this value
    to: int         // Wave of allowed final values
}
```

**ComputeFutureSetPresent:**
1. For each cell with an observation, set `future[i] = obs.to` (target wave)
2. Replace current value with `obs.from`
3. Returns false if observed value not present (invalid constraint)

**ComputeBackwardPotentials:**
Propagates from goal state backwards using rules in reverse.

**ComputeForwardPotentials:**
Propagates from current state forwards.

**Key Algorithm - Potential Propagation:**
```rust
fn compute_potentials(backwards: bool) {
    // Initialize queue with all cells at potential 0
    let mut queue = VecDeque::new();
    for c in 0..C {
        for i in 0..grid_size {
            if potentials[c][i] == 0 {
                queue.push_back((c, x, y, z));
            }
        }
    }
    
    // BFS propagation
    while let Some((value, x, y, z)) = queue.pop_front() {
        let t = potentials[value][i];
        
        for rule in rules {
            // Use ishifts (forward) or oshifts (backward)
            let shifts = if backwards { rule.oshifts[value] } else { rule.ishifts[value] };
            
            for (dx, dy, dz) in shifts {
                let sx = x - dx;
                // ... bounds check ...
                
                if forward_matches(rule, sx, sy, sz, potentials, t, backwards) {
                    apply_forward(rule, sx, sy, sz, potentials, t, &mut queue, backwards);
                }
            }
        }
    }
}
```

### Field (`Field.cs` - 119 lines)

Fields compute distance potentials for heuristic-guided rule selection.

```
Field {
    recompute: bool     // Recompute every turn?
    inversed: bool      // Minimize (false) or maximize (true)?
    essential: bool     // Fail if unreachable?
    zero: int           // Wave of zero-potential cells
    substrate: int      // Wave of traversable cells
}
```

**Compute:**
BFS from all cells matching `zero` wave, spreading through cells matching `substrate` wave.

### Search (`Search.cs` - 294 lines)

A* search through state space to find a path to goal state.

```rust
fn run(present: &[u8], future: &[i32], rules: &[Rule], ...) -> Option<Vec<Vec<u8>>> {
    // Compute potentials
    let bpotentials = compute_backward_potentials(future);
    let fpotentials = compute_forward_potentials(present);
    
    // Check feasibility
    let root_backward = backward_pointwise(bpotentials, present);
    let root_forward = forward_pointwise(fpotentials, future);
    if root_backward < 0 || root_forward < 0 {
        return None;  // Impossible
    }
    
    // A* search
    let mut frontier = PriorityQueue::new();
    let mut visited = HashMap::new();
    let mut database = vec![root_board];
    
    frontier.push(0, root_board.rank());
    
    while let Some(parent_idx) = frontier.pop() {
        let parent = &database[parent_idx];
        
        for child_state in expand_state(parent.state, rules) {
            if let Some(existing_idx) = visited.get(&child_state) {
                // Found shorter path to existing state
                if parent.depth + 1 < database[existing_idx].depth {
                    database[existing_idx].depth = parent.depth + 1;
                    database[existing_idx].parent = parent_idx;
                }
            } else {
                // New state
                let child_backward = backward_pointwise(bpotentials, &child_state);
                let child_forward = forward_pointwise(fpotentials, &child_state);
                
                if child_forward == 0 {
                    // Goal reached!
                    return Some(reconstruct_trajectory());
                }
                
                frontier.push(new_idx, child_board.rank());
            }
        }
    }
    
    None
}
```

**Board.rank():**
```rust
fn rank(&self, random: f64, depth_coefficient: f64) -> f64 {
    if depth_coefficient < 0.0 {
        1000.0 - self.depth as f64  // DFS-like
    } else {
        self.forward_estimate + self.backward_estimate 
            + 2.0 * depth_coefficient * self.depth as f64
    } + 0.0001 * random  // Tie-breaking
}
```

---

## WFC System (`WaveFunctionCollapse.cs` - 327 lines)

Wave Function Collapse maintains a superposition of possible patterns at each cell.

### Wave Structure

```
Wave {
    data: bool[][]          // [grid_pos][pattern] = is pattern allowed?
    compatible: int[][][]   // [grid_pos][pattern][direction] = support count
    
    sumsOfOnes: int[]       // [grid_pos] = remaining patterns
    
    // Shannon entropy tracking
    sumsOfWeights: f64[]
    sumsOfWeightLogWeights: f64[]
    entropies: f64[]
}
```

### Propagator

```
propagator: int[][][]  // [direction][pattern] = compatible patterns
// direction: 0=+x, 1=+y, 2=-x, 3=-y, 4=+z, 5=-z
```

### Algorithm

1. **Observe**: Pick cell with minimum entropy, collapse to single pattern
2. **Propagate**: Remove incompatible patterns from neighbors
3. **Repeat** until all collapsed or contradiction

```rust
fn observe(&mut self, node: usize) {
    let w = &self.wave.data[node];
    
    // Weight by pattern frequency
    let mut distribution = vec![0.0; P];
    for t in 0..P {
        distribution[t] = if w[t] { weights[t] } else { 0.0 };
    }
    
    // Random weighted selection
    let chosen = weighted_random(&distribution);
    
    // Ban all other patterns
    for t in 0..P {
        if w[t] && t != chosen {
            ban(node, t);
        }
    }
}

fn propagate(&mut self) -> bool {
    while let Some((i1, p1)) = stack.pop() {
        let (x1, y1, z1) = index_to_coords(i1);
        
        for d in 0..6 {
            let (x2, y2, z2) = (x1 + DX[d], y1 + DY[d], z1 + DZ[d]);
            // ... bounds/periodic handling ...
            
            let i2 = coords_to_index(x2, y2, z2);
            let compatible_patterns = &propagator[d][p1];
            
            for &t2 in compatible_patterns {
                compatible[i2][t2][d] -= 1;
                if compatible[i2][t2][d] == 0 {
                    ban(i2, t2);
                }
            }
        }
    }
    
    // Check for contradiction
    wave.sumsOfOnes[0] > 0
}

fn ban(&mut self, i: usize, t: usize) {
    wave.data[i][t] = false;
    
    for d in 0..6 {
        compatible[i][t][d] = 0;
    }
    
    stack.push((i, t));
    wave.sumsOfOnes[i] -= 1;
    
    // Update entropy
    if shannon {
        // ... entropy calculation ...
    }
}
```

---

## Convolution (`Convolution.cs` - 190 lines)

Cellular automata based on neighbor counts.

### Kernels

```rust
// 2D Von Neumann (4 neighbors)
const VON_NEUMANN_2D: [i32; 9] = [0, 1, 0, 1, 0, 1, 0, 1, 0];

// 2D Moore (8 neighbors)
const MOORE_2D: [i32; 9] = [1, 1, 1, 1, 0, 1, 1, 1, 1];

// 3D Von Neumann (6 neighbors)
const VON_NEUMANN_3D: [i32; 27] = [
    0, 0, 0,  0, 1, 0,  0, 0, 0,
    0, 1, 0,  1, 0, 1,  0, 1, 0,
    0, 0, 0,  0, 1, 0,  0, 0, 0,
];
```

### ConvolutionRule

```
ConvolutionRule {
    input: byte         // Current cell value
    output: byte        // New cell value
    values: byte[]      // Colors to count
    sums: bool[]        // Allowed sums (e.g., [false, false, false, true] = exactly 3)
    p: double           // Probability
}
```

---

## PathNode (`Path.cs` - 228 lines)

Dijkstra-based path finding between colored regions.

```
PathNode {
    start: int          // Wave of start cells
    finish: int         // Wave of finish cells
    substrate: int      // Wave of traversable cells
    value: byte         // Color to paint path
    inertia: bool       // Prefer straight lines?
    longest: bool       // Find longest path?
    edges: bool         // Allow diagonal (2D edge) movement?
    vertices: bool      // Allow 3D diagonal movement?
}
```

**Algorithm:**
1. BFS from `finish` cells to compute `generations` (distance field)
2. Pick start position (random from shortest or longest distance)
3. Walk from start following gradient to finish

---

## MapNode (`Map.cs` - 115 lines)

Scales grid and applies transformation rules.

```
MapNode {
    newgrid: Grid       // Output grid (different size)
    rules: Rule[]       // Transformation rules
    NX, NY, NZ: int     // Numerators
    DX, DY, DZ: int     // Denominators
}
```

Scale: `new_size = old_size * N / D`

Example: `scale="2 2 1"` doubles X and Y dimensions.

---

## ConvChain (`ConvChain.cs` - 126 lines)

Markov Chain Monte Carlo texture synthesis.

1. Extract N×N patterns from sample with weights
2. Initialize random binary grid on substrate
3. For each cell, compute ratio of weights if toggled
4. Accept/reject based on Metropolis criterion

---

## Interpreter (`Interpreter.cs` - 86 lines)

Main execution engine.

```
Interpreter {
    root: Branch        // Root node (Markov or Sequence)
    current: Branch     // Currently executing branch
    grid: Grid          // Current grid state
    startgrid: Grid     // Initial grid
    
    origin: bool        // Place colored dot at center?
    random: Random      // RNG
    
    changes: List<(x, y, z)>    // Changed cells
    first: List<int>    // changes index at start of each turn
    counter: int        // Turn counter
}
```

**Main Loop:**
```rust
fn run(&mut self, seed: i32, steps: i32) {
    self.random = Random::new(seed);
    self.grid.clear();
    
    if self.origin {
        // Set center cell to color 1
        let center = MX/2 + MY/2 * MX + MZ/2 * MX * MY;
        grid.state[center] = 1;
    }
    
    self.root.reset();
    self.current = &mut self.root;
    
    while self.current.is_some() && (steps <= 0 || counter < steps) {
        self.current.go();
        self.counter += 1;
        self.first.push(changes.len());
    }
}
```

---

## Symmetry System (`SymmetryHelper.cs` - 112 lines)

### 2D Symmetries (8 elements - dihedral group D4)

```rust
// Symmetry subgroups
"()"      = [e]                           // Identity only
"(x)"     = [e, r]                        // Reflect X
"(y)"     = [e, r²]                       // Reflect Y
"(x)(y)"  = [e, r, r², r·r²]              // Reflect both
"(xy+)"   = [e, a, a², a³]                // Rotations only
"(xy)"    = all 8                         // Full symmetry
```

Where:
- `a` = 90° rotation
- `r` = reflection

### 3D Symmetries (48 elements - cubic group)

Similar but with 48 transformations using X, Y, Z rotations and reflection.

---

## XML Model Structure

### Basic Example
```xml
<one values="BW" in="B" out="W"/>
```

### Complex Example
```xml
<sequence values="BPWRG" origin="True">
  <all in="PBB" out="**P"/>
  <markov>
    <one in="RBP" out="GGR"/>
    <one in="GGR" out="RWW"/>
    <one in="P" out="R"/>
  </markov>
  <all in="BBB/BWB" out="BBB/BBB"/>
</sequence>
```

### With Inference
```xml
<one search="True" limit="1000" depthCoefficient="-1.0">
  <rule in="RB" out="BR"/>
  <observe value="D" from="B" to="RW"/>
</one>
```

---

## Key Implementation Considerations for Rust

### Memory Layout
- Grid state is flat: `Vec<u8>` with `index = x + y * MX + z * MX * MY`
- Rules use waves (bitmasks): `u32` or `u64` for up to 64 colors

### Performance Critical
1. **Pattern matching** - ishifts pre-computation is essential
2. **Change tracking** - incremental updates via `changes` list
3. **WFC propagation** - stack-based with support counting

### Lua Integration Points
1. Model loading/parsing
2. Custom node types
3. Rule definition DSL
4. Callbacks for visualization

### Error Handling
- XML parsing failures with line numbers
- Invalid color references
- Pattern size mismatches
- Contradiction detection in WFC

---

## File-by-File Summary

| File | Lines | Purpose |
|------|-------|---------|
| Grid.cs | 139 | World state management |
| Rule.cs | 283 | Rewrite rule representation |
| Node.cs | 110 | Base node + Branch/Sequence/Markov |
| RuleNode.cs | 219 | Base for rule-based nodes |
| OneNode.cs | 140 | Random single match |
| AllNode.cs | 108 | All non-conflicting matches |
| ParallelNode.cs | 50 | All matches (with conflicts) |
| Observation.cs | 184 | Future constraints + potentials |
| Search.cs | 294 | A* state space search |
| Field.cs | 119 | Distance field computation |
| WaveFunctionCollapse.cs | 327 | WFC base + Wave structure |
| TileModel.cs | 331 | Tile-based WFC |
| OverlapModel.cs | 180 | Overlapping WFC |
| Convolution.cs | 190 | Cellular automata |
| ConvChain.cs | 126 | MCMC texture synthesis |
| Path.cs | 228 | Dijkstra pathfinding |
| Map.cs | 115 | Grid scaling/transformation |
| SymmetryHelper.cs | 112 | Symmetry groups |
| Interpreter.cs | 86 | Main execution loop |
| Helper.cs | 121 | Utility functions |
| ArrayHelper.cs | 57 | Array creation utilities |
| XMLHelper.cs | 39 | XML parsing utilities |
| Program.cs | 80 | Entry point |
| Graphics.cs | 224 | Rendering (not needed) |
| VoxHelper.cs | 141 | VOX file I/O |
| GUI.cs | 330 | GUI (not needed) |

**Total: ~4333 lines** (excluding Graphics/GUI/VoxHelper = ~3600 lines of core logic)
