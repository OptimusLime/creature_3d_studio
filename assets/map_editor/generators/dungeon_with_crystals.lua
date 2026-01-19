-- Example: Dungeon with scattered crystals
--
-- Demonstrates composed generators using Sequential:
-- 1. Generate maze structure using Markov Jr.
-- 2. Scatter crystals on floor cells
--
-- Usage: Set this as the generator via MCP or copy to generator.lua

local generators = require("lib.generators")

-- Create a sequential generator:
-- Step 1: Markov maze generation
-- Step 2: Scatter crystals on floor (material 1)
return generators.sequential({
    -- Markov maze generator
    mj.load_model("MarkovJunior/models/MazeGrowth.xml"),
    
    -- Scatter crystals on floor cells
    generators.scatter({
        material = 3,     -- Crystal material ID
        target = 1,       -- Only scatter on floor (W = 1)
        density = 0.05,   -- 5% chance per cell
    })
})
