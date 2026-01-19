-- Example: Dungeon with scattered crystals
--
-- Demonstrates composed generators using Sequential:
-- 1. Generate maze structure using Markov Jr.
-- 2. Scatter crystals on floor cells
--
-- Material mapping (MJ grid value + 1 = material ID):
--   B (Black/walls)  = value 0 → material 1 (black)
--   W (White/paths)  = value 1 → material 2 (off-white)
--   A (Alive/growth) = value 2 → material 3 (gray)

local generators = require("lib.generators")

-- Create a sequential generator:
-- Step 1: Markov maze generation (32x32 to match grid size)
-- Step 2: Scatter crystals on floor (material 2 = path cells)
return generators.sequential({
    -- Markov maze generator - use load_model_xml with explicit size
    mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", { size = 32 }),
    
    -- Scatter crystals on floor cells
    generators.scatter({
        material = 10,    -- Crystal (bright green)
        target = 2,       -- Scatter on floor (W → material 2)
        density = 0.05,   -- 5% chance per cell
    })
})
