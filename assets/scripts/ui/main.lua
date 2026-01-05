-- Main UI script for Creature 3D Studio
-- Demonstrates Lua-driven ImGui UI controlling physics scene
-- Now includes MarkovJunior procedural generation!

scene.print("main.lua loaded")

-- MarkovJunior model (created once, reused)
local mj_model = nil
local current_seed = 42
local generated = false
local model_type = "maze3d"  -- "growth", "maze3d", "dungeon"

-- Verification state
local verification_models = nil
local current_verification_idx = 1
local verification_model = nil
local verification_generated = false

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
    
    -- MJ Verification Window (Phase 4)
    imgui.window("MJ Verification", function()
        imgui.text("2D Model Verification")
        imgui.separator()
        
        -- Load models list on first run
        if not verification_models then
            verification_models = mj.list_models_with_refs()
            if #verification_models > 0 then
                scene.print(string.format("Found %d models with reference images", #verification_models))
            else
                scene.print("No models with reference images found")
            end
        end
        
        -- Show model count
        imgui.text(string.format("Models: %d", #verification_models))
        
        if #verification_models == 0 then
            imgui.text("No verification models found!")
            imgui.text("Check: assets/reference_images/mj/")
            return
        end
        
        -- Model selection buttons
        imgui.separator()
        imgui.text("Select Model:")
        
        local models_per_row = 4
        for i, model_info in ipairs(verification_models) do
            if i > 1 and (i - 1) % models_per_row ~= 0 then
                imgui.same_line()
            end
            
            local is_selected = (i == current_verification_idx)
            local label = model_info.name
            if is_selected then
                label = "[" .. label .. "]"
            end
            
            if imgui.button(label) then
                current_verification_idx = i
                verification_model = nil
                verification_generated = false
                scene.print("Selected: " .. model_info.name)
            end
        end
        
        imgui.separator()
        
        -- Current model info
        local current_info = verification_models[current_verification_idx]
        if current_info then
            imgui.text("Model: " .. current_info.name)
            imgui.text("Size: " .. tostring(current_info.size) .. "x" .. tostring(current_info.size))
            imgui.text("Reference: " .. current_info.ref_path)
            
            imgui.separator()
            
            -- Generate button
            if imgui.button("Generate 2D") then
                scene.print("Loading: " .. current_info.xml_path)
                
                -- Load model with correct size from models.xml
                verification_model = mj.load_model_xml(current_info.xml_path, {
                    size = current_info.size,
                    mz = 1
                })
                
                -- Run with seed 0 for reproducibility
                local max_steps = current_info.size * current_info.size * 2
                local steps = verification_model:run(0, max_steps)
                
                local grid = verification_model:grid()
                local nonzero = grid:count_nonzero()
                
                -- Save our output
                local output_path = "screenshots/verify_" .. current_info.name .. ".png"
                grid:render_to_png(output_path, 2)
                
                verification_generated = true
                scene.print(string.format("Generated %s: %d steps, %d cells", 
                    current_info.name, steps, nonzero))
                scene.print("Saved to: " .. output_path)
                scene.print("Reference: " .. current_info.ref_path)
            end
            
            imgui.same_line()
            
            if imgui.button("Run Full") then
                scene.print("Running FULL: " .. current_info.xml_path)
                
                verification_model = mj.load_model_xml(current_info.xml_path, {
                    size = current_info.size,
                    mz = 1
                })
                
                -- Run to completion
                local steps = verification_model:run(0, 0)
                
                local grid = verification_model:grid()
                local nonzero = grid:count_nonzero()
                
                -- Save our output
                local output_path = "screenshots/verify_" .. current_info.name .. "_full.png"
                grid:render_to_png(output_path, 2)
                
                verification_generated = true
                scene.print(string.format("FULL %s: %d steps, %d cells", 
                    current_info.name, steps, nonzero))
                scene.print("Saved to: " .. output_path)
            end
            
            if verification_generated and verification_model then
                imgui.separator()
                local grid = verification_model:grid()
                imgui.text(string.format("Grid: %dx%dx%d", 
                    grid:size()[1], grid:size()[2], grid:size()[3]))
                imgui.text(string.format("Non-zero: %d", grid:count_nonzero()))
                imgui.text(string.format("Steps: %d", verification_model:counter()))
                
                imgui.separator()
                imgui.text("Compare images in screenshots/:")
                imgui.text("  Our: verify_" .. current_info.name .. ".png")
                imgui.text("  Ref: " .. current_info.ref_path)
            end
        end
    end)
end
