-- test_darkworld.lua
-- Dark world test scene for colored moon lighting and point lights.
--
-- Features:
-- - Dark rocky terrain
-- - Glowing crystals and orbs of different colors
-- - Structures that catch light and cast shadows
-- - Purple and orange color palette (80s dark fantasy)

print("[darkworld] Creating dark world test scene...")

-- Colors (RGB 0-255)
local OBSIDIAN = {20, 15, 25}         -- Dark purple-black rock
local DARK_STONE = {35, 30, 40}       -- Slightly lighter purple stone
local PURPLE_CRYSTAL = {180, 50, 220} -- Bright purple
local ORANGE_CRYSTAL = {255, 140, 40} -- Bright orange
local CYAN_CRYSTAL = {50, 220, 255}   -- Cyan accent
local MAGENTA_CRYSTAL = {255, 50, 180}-- Hot pink
local DARK_METAL = {45, 40, 50}       -- For ruins

-- Helper to place a voxel
local function put(x, y, z, color, emission)
    emission = emission or 0
    place_voxel(x, y, z, color[1], color[2], color[3], emission)
end

-- Helper for filled box
local function box(x1, y1, z1, x2, y2, z2, color, emission)
    for x = x1, x2 do
        for y = y1, y2 do
            for z = z1, z2 do
                put(x, y, z, color, emission or 0)
            end
        end
    end
end

local voxel_count = 0

-- Ground plane - dark rocky surface with slight variations
print("[darkworld] Creating ground...")
for x = 0, 31 do
    for z = 0, 31 do
        -- Vary the ground height slightly
        local height_noise = math.floor(math.sin(x * 0.3) * math.cos(z * 0.4) + 0.5)
        local base_y = height_noise
        
        -- Use darker stone at edges, obsidian in most places
        local color = OBSIDIAN
        if (x + z) % 7 == 0 then
            color = DARK_STONE
        end
        
        for y = 0, base_y do
            put(x, y, z, color, 0)
            voxel_count = voxel_count + 1
        end
    end
end

-- Raised platform in center (altar-like structure)
print("[darkworld] Creating central altar...")
box(12, 0, 12, 19, 2, 19, DARK_STONE, 0)
box(13, 2, 13, 18, 3, 18, DARK_STONE, 0)
box(14, 3, 14, 17, 4, 17, OBSIDIAN, 0)
voxel_count = voxel_count + (8*3*8) + (6*2*6) + (4*2*4)

-- Ruined pillars at corners of altar (for shadow testing)
print("[darkworld] Creating pillars...")
local pillar_positions = {
    {x = 12, z = 12},
    {x = 19, z = 12},
    {x = 12, z = 19},
    {x = 19, z = 19},
}

for _, pos in ipairs(pillar_positions) do
    -- Pillar base
    for y = 0, 8 do
        put(pos.x, y, pos.z, DARK_METAL, 0)
        voxel_count = voxel_count + 1
    end
    -- Broken top (some pillars shorter)
    if pos.x == 12 and pos.z == 12 then
        for y = 9, 12 do
            put(pos.x, y, pos.z, DARK_METAL, 0)
            voxel_count = voxel_count + 1
        end
    end
end

-- === GLOWING ELEMENTS (point light sources) ===

-- Central altar orb (orange - matches one moon)
print("[darkworld] Creating glowing orbs...")
put(15, 6, 15, ORANGE_CRYSTAL, 255)
put(16, 6, 15, ORANGE_CRYSTAL, 255)
put(15, 6, 16, ORANGE_CRYSTAL, 255)
put(16, 6, 16, ORANGE_CRYSTAL, 255)
put(15, 7, 15, ORANGE_CRYSTAL, 255)
put(16, 7, 16, ORANGE_CRYSTAL, 255)
voxel_count = voxel_count + 6

-- Purple crystal cluster (left side)
put(5, 1, 8, PURPLE_CRYSTAL, 220)
put(5, 2, 8, PURPLE_CRYSTAL, 220)
put(5, 3, 8, PURPLE_CRYSTAL, 220)
put(6, 1, 9, PURPLE_CRYSTAL, 200)
put(6, 2, 9, PURPLE_CRYSTAL, 200)
put(4, 1, 7, PURPLE_CRYSTAL, 180)
voxel_count = voxel_count + 6

-- Cyan crystal (front right)
put(26, 1, 5, CYAN_CRYSTAL, 240)
put(26, 2, 5, CYAN_CRYSTAL, 240)
put(26, 3, 5, CYAN_CRYSTAL, 240)
put(26, 4, 5, CYAN_CRYSTAL, 240)
put(27, 1, 6, CYAN_CRYSTAL, 200)
voxel_count = voxel_count + 5

-- Magenta crystal (back)
put(10, 1, 26, MAGENTA_CRYSTAL, 230)
put(10, 2, 26, MAGENTA_CRYSTAL, 230)
put(10, 3, 26, MAGENTA_CRYSTAL, 230)
put(11, 1, 27, MAGENTA_CRYSTAL, 200)
put(11, 2, 27, MAGENTA_CRYSTAL, 200)
voxel_count = voxel_count + 5

-- Scattered small glowing stones
local scattered_glows = {
    {x = 3, y = 1, z = 20, color = ORANGE_CRYSTAL, emission = 150},
    {x = 28, y = 1, z = 22, color = PURPLE_CRYSTAL, emission = 150},
    {x = 22, y = 1, z = 28, color = CYAN_CRYSTAL, emission = 150},
    {x = 8, y = 1, z = 3, color = MAGENTA_CRYSTAL, emission = 150},
    {x = 25, y = 1, z = 10, color = ORANGE_CRYSTAL, emission = 180},
    {x = 2, y = 1, z = 15, color = PURPLE_CRYSTAL, emission = 160},
}

for _, glow in ipairs(scattered_glows) do
    put(glow.x, glow.y, glow.z, glow.color, glow.emission)
    voxel_count = voxel_count + 1
end

-- Floating rock with crystal (demonstrates lighting from above)
print("[darkworld] Creating floating rock...")
box(20, 8, 8, 23, 9, 11, DARK_STONE, 0)
put(21, 10, 9, ORANGE_CRYSTAL, 255)
put(22, 10, 10, ORANGE_CRYSTAL, 255)
voxel_count = voxel_count + (4*2*4) + 2

-- Low wall for shadow testing
print("[darkworld] Creating shadow test wall...")
box(6, 1, 14, 6, 4, 18, DARK_STONE, 0)
voxel_count = voxel_count + (1*4*5)

print("[darkworld] Created dark world with " .. voxel_count .. " voxels")
print("[darkworld] Features: altar, pillars, crystals, floating rock, wall")
print("[darkworld] Glowing sources: orange altar, purple/cyan/magenta crystals")
