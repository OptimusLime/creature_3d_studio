-- Main UI script for Creature 3D Studio
-- Demonstrates Lua-driven ImGui UI controlling physics scene
-- Now includes MarkovJunior procedural generation!

scene.print("main.lua loaded")

-- MarkovJunior model (created once, reused)
local mj_model = nil
local current_seed = 42
local generated = false
local model_type = "maze3d"  -- "growth", "maze3d", "dungeon"

-- Create different types of 3D models
local function create_mj_model(type)
    if type == "growth" then
        -- Simple organic growth from center
        local builder = mj.create_model({
            values = "BW",
            size = {16, 16, 16},
            origin = true
        })
        builder:one("WB", "WW")
        return builder:build()
        
    elseif type == "maze3d" then
        -- 3D maze with corridors - uses same pattern as MazeGrowth but in 3D
        -- WBB->WAW creates corridors, A marks walls
        local builder = mj.create_model({
            values = "BWA",
            size = {17, 17, 17},  -- Odd size for proper maze
            origin = true
        })
        -- Maze growth: extend path through empty space, leaving walls
        builder:one("WBB", "WAW")
        return builder:build()
        
    elseif type == "dungeon" then
        -- Dungeon-like structure with rooms
        -- Grows in a more structured way
        local builder = mj.create_model({
            values = "BWRG",  -- Black, White(floor), Red(walls), Green(doors)
            size = {24, 24, 8},
            origin = true
        })
        -- Create floor expansion
        builder:one("WB", "WW")
        return builder:build()
    end
    
    -- Default to growth
    local builder = mj.create_model({
        values = "BW",
        size = {16, 16, 16},
        origin = true
    })
    builder:one("WB", "WW")
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
        
        -- Model type selection
        imgui.text("Model: " .. model_type)
        if imgui.button("Growth") then
            model_type = "growth"
            mj_model = nil
            generated = false
            scene.print("Switched to Growth model")
        end
        imgui.same_line()
        if imgui.button("Maze3D") then
            model_type = "maze3d"
            mj_model = nil
            generated = false
            scene.print("Switched to Maze3D model")
        end
        imgui.same_line()
        if imgui.button("Dungeon") then
            model_type = "dungeon"
            mj_model = nil
            generated = false
            scene.print("Switched to Dungeon model")
        end
        
        imgui.separator()

        if imgui.button("Generate") then
            -- Create model if needed
            if not mj_model then
                mj_model = create_mj_model(model_type)
                scene.print("Created " .. model_type .. " model")
            end

            -- Generate with new random seed
            current_seed = math.random(1, 999999)
            
            local max_steps = 3000
            if model_type == "maze3d" then max_steps = 5000 end
            if model_type == "dungeon" then max_steps = 4000 end
            
            mj_model:run_animated({
                seed = current_seed,
                max_steps = max_steps,
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
                mj_model = create_mj_model(model_type)
                scene.print("Created " .. model_type .. " model")
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
            generated = true
        end

        imgui.separator()
        imgui.text(string.format("Seed: %d", current_seed))
        if mj_model and generated then
            imgui.text(string.format("Counter: %d", mj_model:counter()))
        end

        imgui.separator()
        
        -- Save PNG button
        if imgui.button("Save PNG") then
            if mj_model and generated then
                local grid = mj_model:grid()
                local filename = string.format("screenshots/mj_generated_%d.png", current_seed)
                local success = grid:render_to_png(filename)
                if success then
                    scene.print(string.format("Saved to %s", filename))
                end
            else
                scene.print("Generate first before saving!")
            end
        end
    end)
end
