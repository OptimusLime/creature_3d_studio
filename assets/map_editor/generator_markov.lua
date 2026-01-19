-- Markov Jr. Generator Script
-- Uses the mj module to load and run MarkovJunior models step-by-step.
--
-- Protocol: init(ctx), step(ctx) -> bool, reset()
--
-- This script demonstrates using mj.load_model() to load an XML model
-- and running it step-by-step with playback controls.

local Generator = {}
local model = nil
local seed = nil

-- Mapping from Markov grid values to material IDs
-- B=0 (black/wall), W=1 (white/floor), A=2 (visited)
local VALUE_TO_MATERIAL = {
    [0] = 1,  -- B -> first palette material (wall)
    [1] = 2,  -- W -> second palette material (floor)
    [2] = 3,  -- A -> third palette material (if available)
}

function Generator:init(ctx)
    -- Use MazeGrowth - a simple but visually interesting 2D maze generator
    model = mj.load_model("MarkovJunior/models/MazeGrowth.xml")
    
    -- Use current time as seed for variety
    seed = os.time()
    model:reset(seed)
    
    print("MarkovJunior model loaded, seed: " .. seed)
end

function Generator:step(ctx)
    if model == nil then
        return true -- no model, done
    end
    
    -- Check if model is still running
    if not model:is_running() then
        return true -- done
    end
    
    -- Execute one step of the Markov model
    local made_progress = model:step()
    
    -- Copy the Markov grid to the voxel buffer
    local grid = model:grid()
    local size = grid:size()
    
    -- Copy grid values to buffer (clamped to buffer dimensions)
    local max_x = math.min(size[1], ctx.width)
    local max_y = math.min(size[2], ctx.height)
    
    for y = 0, max_y - 1 do
        for x = 0, max_x - 1 do
            local val = grid:get(x, y, 0)
            -- Map Markov value to material ID, use palette if available
            local mat_id = 1
            if ctx.palette[val + 1] then
                mat_id = ctx.palette[val + 1]
            elseif VALUE_TO_MATERIAL[val] then
                mat_id = VALUE_TO_MATERIAL[val]
            end
            ctx:set_voxel(x, y, mat_id)
        end
    end
    
    -- Return true when model is done
    return not model:is_running()
end

function Generator:reset()
    if model then
        -- Reset with a new seed
        seed = os.time()
        model:reset(seed)
    end
end

return Generator
