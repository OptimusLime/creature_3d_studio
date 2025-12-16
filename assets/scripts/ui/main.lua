-- Main UI script for Creature 3D Studio
-- Demonstrates Lua-driven ImGui UI controlling physics scene

scene.print("main.lua loaded")

function on_draw()
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
end
