-- test_island.lua
-- A floating island test scene for the deferred rendering pipeline.
--
-- Features:
-- - Grass top layer (green)
-- - Dirt middle layer (brown)
-- - Stone base (gray)
-- - Glowing crystals (cyan/magenta with high emission)
-- - A small tree

print("[island] Creating floating island test scene...")

-- Colors (RGB 0-255)
local GRASS = {34, 139, 34}      -- Forest green
local DIRT = {139, 90, 43}       -- Saddle brown
local STONE = {128, 128, 128}    -- Gray
local CRYSTAL_CYAN = {0, 255, 255}
local CRYSTAL_MAGENTA = {255, 0, 255}
local TRUNK = {101, 67, 33}      -- Dark brown
local LEAVES = {50, 205, 50}     -- Lime green

-- Helper to place a voxel
local function put(x, y, z, color, emission)
    emission = emission or 0
    place_voxel(x, y, z, color[1], color[2], color[3], emission)
end

-- Island parameters
local island_radius = 6
local island_center_x = 8
local island_center_z = 8

-- Generate island terrain (roughly circular)
local voxel_count = 0

for x = 0, 15 do
    for z = 0, 15 do
        -- Distance from center
        local dx = x - island_center_x
        local dz = z - island_center_z
        local dist = math.sqrt(dx * dx + dz * dz)
        
        -- Island shape: taller in center, tapers at edges
        if dist <= island_radius then
            local height_factor = 1 - (dist / island_radius)
            local max_height = math.floor(3 + height_factor * 4)  -- 3-7 blocks tall
            local base_y = 4  -- Start at y=4 to center vertically
            
            for y = 0, max_height do
                local world_y = base_y + y
                
                if y == max_height then
                    -- Top layer: grass
                    put(x, world_y, z, GRASS, 0)
                elseif y >= max_height - 2 then
                    -- Upper layers: dirt
                    put(x, world_y, z, DIRT, 0)
                else
                    -- Lower layers: stone
                    put(x, world_y, z, STONE, 0)
                end
                
                voxel_count = voxel_count + 1
            end
        end
    end
end

-- Add glowing crystals emerging from the island
local crystals = {
    {x = 10, y = 9, z = 8, color = CRYSTAL_CYAN},
    {x = 10, y = 10, z = 8, color = CRYSTAL_CYAN},
    {x = 6, y = 10, z = 10, color = CRYSTAL_MAGENTA},
    {x = 6, y = 11, z = 10, color = CRYSTAL_MAGENTA},
    {x = 6, y = 12, z = 10, color = CRYSTAL_MAGENTA},
    {x = 8, y = 11, z = 5, color = CRYSTAL_CYAN},
}

for _, crystal in ipairs(crystals) do
    put(crystal.x, crystal.y, crystal.z, crystal.color, 200)  -- High emission
    voxel_count = voxel_count + 1
end

-- Add a small tree on top
local tree_x, tree_z = 8, 8

-- Find the top of the island at tree position
local tree_base_y = 11  -- Grass level at center

-- Trunk (3 blocks tall)
for y = 0, 2 do
    put(tree_x, tree_base_y + y, tree_z, TRUNK, 0)
    voxel_count = voxel_count + 1
end

-- Leaves (simple 3x3x2 canopy)
local canopy_y = tree_base_y + 3
for dx = -1, 1 do
    for dz = -1, 1 do
        for dy = 0, 1 do
            -- Skip corners on top layer for rounder shape
            if dy == 1 and math.abs(dx) + math.abs(dz) == 2 then
                -- skip corner
            else
                put(tree_x + dx, canopy_y + dy, tree_z + dz, LEAVES, 0)
                voxel_count = voxel_count + 1
            end
        end
    end
end

-- Add one glowing leaf at the top
put(tree_x, canopy_y + 2, tree_z, LEAVES, 100)  -- Slight glow
voxel_count = voxel_count + 1

print("[island] Created floating island with " .. voxel_count .. " voxels")
print("[island] Features: terrain, crystals, tree")
