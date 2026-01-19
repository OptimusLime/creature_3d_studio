-- Step highlight visualizer for 2D map editor
-- Draws a highlight at the current generation position
--
-- The step info is passed through the render context:
--   ctx.step_x, ctx.step_y - current step position (nil if none)
--   ctx.step_material_id - material placed
--   ctx.step_completed - whether generation is done
--   ctx:has_step_info() - returns true if step info is available

local Visualizer = {}

function Visualizer:render(ctx, pixels)
    -- Don't render if no step info
    if not ctx:has_step_info() then
        return
    end

    local x = ctx.step_x
    local y = ctx.step_y

    -- Highlight color: bright yellow for active, green for completed
    local r, g, b, a
    if ctx.step_completed then
        r, g, b, a = 0, 255, 0, 255  -- Green when done
    else
        r, g, b, a = 255, 255, 0, 255  -- Yellow when active
    end

    -- Draw a larger highlight box (3x3) for better visibility
    for dy = -1, 1 do
        for dx = -1, 1 do
            local px = x + dx
            local py = y + dy
            if px >= 0 and px < ctx.width and py >= 0 and py < ctx.height then
                -- Center pixel is brightest, edges are dimmer
                local alpha = (dx == 0 and dy == 0) and 255 or 180
                pixels:blend_pixel(px, py, r, g, b, alpha)
            end
        end
    end

    -- Draw outer ring for even more visibility (5x5 with corners)
    local outer_a = 100
    for dy = -2, 2 do
        for dx = -2, 2 do
            -- Skip inner 3x3 and corners of 5x5
            local is_inner = (dx >= -1 and dx <= 1 and dy >= -1 and dy <= 1)
            local is_corner = (math.abs(dx) == 2 and math.abs(dy) == 2)
            if not is_inner and not is_corner then
                local px = x + dx
                local py = y + dy
                if px >= 0 and px < ctx.width and py >= 0 and py < ctx.height then
                    pixels:blend_pixel(px, py, r, g, b, outer_a)
                end
            end
        end
    end
end

return Visualizer
