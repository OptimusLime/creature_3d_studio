-- Phase 7 fog test: single voxel at center
-- 
-- The example will spawn this mesh at multiple z positions to test fog.
-- All voxels have same color and no emission to isolate fog effect.
--
-- Expected result: near voxel full color, far voxel heavily fogged (purple tint)

print("Phase 7 test: single white voxel (spawned at multiple depths by example)")

-- Single white voxel at center, no emission
place_voxel(8, 8, 8, 255, 255, 255, 0)
