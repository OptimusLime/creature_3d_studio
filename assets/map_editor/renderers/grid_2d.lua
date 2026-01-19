-- Base grid renderer for 2D map editor
-- Renders voxels as solid colors from material palette

local Layer = {}

function Layer:render(ctx, pixels)
  -- Iterate over all cells and render them
  for y = 0, ctx.height - 1 do
    for x = 0, ctx.width - 1 do
      local mat_id = ctx:get_voxel(x, y)
      
      if mat_id == 0 then
        -- Empty cell: dark gray
        pixels:set_pixel(x, y, 30, 30, 30, 255)
      else
        -- Get material color
        local r, g, b = ctx:get_material_color(mat_id)
        if r then
          -- Convert from 0-1 to 0-255
          pixels:set_pixel(x, y, 
            math.floor(r * 255), 
            math.floor(g * 255), 
            math.floor(b * 255), 
            255)
        else
          -- Unknown material: magenta
          pixels:set_pixel(x, y, 255, 0, 255, 255)
        end
      end
    end
  end
end

return Layer
