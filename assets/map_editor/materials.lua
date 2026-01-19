-- Map Editor Material Definitions
-- Tags enable search by category: search "natural" finds stone, dirt, grass
--
-- IDs 1-3 are mapped from MarkovJunior grid values (val + 1):
--   B (Black/walls)  = value 0 → material 1
--   W (White/paths)  = value 1 → material 2
--   A (Alive/growth) = value 2 → material 3
--
-- Colors match MJ's palette.xml for consistency
return {
    -- MJ-mapped materials (match palette.xml colors)
    { id = 1, name = "wall",   color = {0.0, 0.0, 0.0}, tags = {"mj", "structure"} },     -- B: Black
    { id = 2, name = "floor",  color = {1.0, 0.945, 0.91}, tags = {"mj", "structure"} },  -- W: Off-white (0xFF, 0xF1, 0xE8)
    { id = 3, name = "growth", color = {0.76, 0.765, 0.78}, tags = {"mj", "structure"} }, -- A: Light gray (0xC2, 0xC3, 0xC7)
    
    -- Additional materials for generators
    { id = 4, name = "crystal", color = {0.0, 0.894, 0.212}, tags = {"ore", "decoration"} }, -- Bright green
    { id = 5, name = "stone",   color = {0.5, 0.5, 0.5}, tags = {"natural", "terrain"} },
    { id = 6, name = "dirt",    color = {0.6, 0.4, 0.2}, tags = {"natural", "terrain"} },
}
