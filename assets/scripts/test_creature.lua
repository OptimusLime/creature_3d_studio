-- Test creature script: 5-voxel cross pattern
-- This demonstrates Lua-based voxel placement for Phase 3 verification.
--
-- Pattern (top-down view, Y is up):
--        +Z (yellow)
--          |
--   -X (blue) -- center (red) -- +X (green)
--          |
--        -Z (cyan)

-- Center voxel at origin (0, 0, 0) - red
place_voxel(0, 0, 0, 255, 0, 0, 0)

-- +X direction - green
place_voxel(1, 0, 0, 0, 255, 0, 0)

-- -X direction - blue
-- Note: Lua uses 1-based indexing but our coords are absolute
-- We need to handle negative coords... but chunk coords are 0-15
-- So we'll offset everything to center in the chunk

-- Actually, let's center the pattern at (8, 8, 8) so all coords are valid
-- Clear and redo:

-- Center voxel at (8, 8, 8) - RED
place_voxel(8, 8, 8, 255, 0, 0, 0)

-- +X direction (9, 8, 8) - GREEN  
place_voxel(9, 8, 8, 0, 255, 0, 0)

-- -X direction (7, 8, 8) - BLUE
place_voxel(7, 8, 8, 0, 0, 255, 0)

-- +Z direction (8, 8, 9) - YELLOW
place_voxel(8, 8, 9, 255, 255, 0, 0)

-- -Z direction (8, 8, 7) - CYAN
place_voxel(8, 8, 7, 0, 255, 255, 0)

-- Clear the origin voxels we placed first (they were at wrong coords)
clear_voxel(0, 0, 0)
clear_voxel(1, 0, 0)

print("Test creature loaded: 5-voxel cross pattern centered at (8, 8, 8)")
