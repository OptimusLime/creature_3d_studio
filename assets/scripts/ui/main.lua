-- Main UI script for Creature 3D Studio
-- Demonstrates Lua-driven ImGui UI controlling physics scene
-- Now includes MarkovJunior procedural generation!

scene.print("main.lua loaded")

-- MarkovJunior model (created once, reused)
local mj_model = nil
local current_seed = 42
local generated = false

-- Create a simple growth model
local function create_mj_model()
    local builder = mj.create_model({
        values = "BW",
        size = {16, 16, 16},
        origin = true
    })
    builder:one("WB", "WW")  -- Growth rule: white expands into adjacent black
    return builder:build()
end

function on_draw()
    -- Physics Controls window
    imgui.window("Lua UI", function()
        imgui.text("Physics Controls (from Lua)")
        imgui.separator()

        if imgui.button("Spawn Cube") then
            -- Spawn at random position above ground
            local x = math.random() * 6 - 3  -- -3 to 3
            local z = math.random() * 6 - 3  -- -3 to 3
            local y = math.random() * 5 + 3  -- 3 to 8
            scene.spawn_cube(x, y, z)
            scene.print(string.format("Spawned cube at (%.1f, %.1f, %.1f)", x, y, z))
        end

        imgui.same_line()

        if imgui.button("Clear All") then
            scene.clear()
            scene.print("Cleared all cubes")
        end
    end)

    -- MarkovJunior Controls window
    imgui.window("MarkovJunior", function()
        imgui.text("Procedural Generation Demo")
        imgui.separator()

        if imgui.button("Generate") then
            -- Create model if needed
            if not mj_model then
                mj_model = create_mj_model()
                scene.print("Created MarkovJunior model")
            end

            -- Generate with new random seed
            current_seed = math.random(1, 999999)
            
            mj_model:run_animated({
                seed = current_seed,
                max_steps = 2000,
                on_complete = function(grid, steps)
                    local world = grid:to_voxel_world()
                    scene.set_voxel_world(world)
                    scene.print(string.format("Generated %d voxels in %d steps (seed: %d)", 
                        grid:count_nonzero(), steps, current_seed))
                    generated = true
                end
            })
        end

        imgui.same_line()

        if imgui.button("Step x100") then
            if not mj_model then
                mj_model = create_mj_model()
                scene.print("Created MarkovJunior model")
            end
            
            if not generated then
                mj_model:reset(current_seed)
            end
            
            for i = 1, 100 do
                if not mj_model:step() then break end
            end
            
            local grid = mj_model:grid()
            local world = grid:to_voxel_world()
            scene.set_voxel_world(world)
            scene.print(string.format("Stepped: %d voxels", grid:count_nonzero()))
        end

        imgui.separator()
        imgui.text(string.format("Seed: %d", current_seed))
        if mj_model and generated then
            imgui.text(string.format("Counter: %d", mj_model:counter()))
        end
    end)
end
