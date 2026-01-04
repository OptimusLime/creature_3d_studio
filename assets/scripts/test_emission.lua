-- Phase 5 test: Emission brightness gradient
-- 4 white voxels in a row with increasing emission values
--
-- Expected visual: leftmost darkest, rightmost brightest
-- All voxels are white (255, 255, 255) but with different emission

-- Voxel positions: row along X axis at y=8, z=8
-- Spacing of 2 units for clear visibility

-- Emission 0 (no glow) - should look like normal lit white
place_voxel(5, 8, 8, 255, 255, 255, 0)

-- Emission 64 (25% glow) - slightly brighter
place_voxel(7, 8, 8, 255, 255, 255, 64)

-- Emission 128 (50% glow) - noticeably brighter
place_voxel(9, 8, 8, 255, 255, 255, 128)

-- Emission 255 (full glow) - brightest, should not clip to pure white
place_voxel(11, 8, 8, 255, 255, 255, 255)

print("Phase 5 test: 4 white voxels with emission gradient (0, 64, 128, 255)")
