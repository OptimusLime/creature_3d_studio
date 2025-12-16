-- Main UI script
tools.print("main.lua loaded")

local click_count = 0

function on_draw()
    imgui.window("Lua UI", function()
        imgui.text("Hello from Lua!")
        imgui.separator()
        imgui.text("Click count: " .. click_count)
        
        if imgui.button("Click me!") then
            click_count = click_count + 1
            tools.print("Button clicked! Count: " .. click_count)
        end
        
        imgui.same_line()
        
        if imgui.button("Reset") then
            click_count = 0
            tools.print("Counter reset")
        end
    end)
end
