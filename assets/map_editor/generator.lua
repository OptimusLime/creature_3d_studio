-- Example: Dungeon with scattered crystals
--
-- Demonstrates composed generators using Sequential:
-- 1. Generate maze structure using Markov Jr.
-- 2. Scatter crystals on floor cells
--
-- Usage: Set this as the generator via MCP or copy to generator.lua

local generators = require("lib.generators")

-- Create a sequential generator:
-- Step 1: Markov maze generation (32x32 to match grid size)
-- Step 2: Scatter crystals on floor (material 2, since W=1 maps to material 2)
return generators.sequential({
    -- Markov maze generator - use load_model_xml with explicit size
    mj.load_model_xml("MarkovJunior/models/MazeGrowth.xml", { size = 32 }),
    
    -- Scatter crystals on floor cells
    generators.scatter({
        material = 3,     -- Crystal material ID
        target = 2,       -- Scatter on floor (W=1 -> material 2)
        density = 0.05,   -- 5% chance per cell
    })
})
