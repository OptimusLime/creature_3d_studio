-- Map Editor Generator Script
-- Protocol: init(ctx), step(ctx) -> bool, reset()
--
-- ctx provides:
--   ctx.width, ctx.height - buffer dimensions
--   ctx:set_voxel(x, y, material_id) - write to buffer
--   ctx:get_voxel(x, y) -> material_id - read from buffer
--   ctx.material_a, ctx.material_b - selected materials

local Generator = {}

function Generator:init(ctx)
    self.x = 0
    self.y = 0
end

function Generator:step(ctx)
    if self.y >= ctx.height then
        return true -- done
    end
    
    -- Checkerboard pattern: alternate materials based on (x + y) % 2
    local mat = ((self.x + self.y) % 2 == 0) and ctx.material_a or ctx.material_b
    ctx:set_voxel(self.x, self.y, mat)
    
    -- Advance to next cell
    self.x = self.x + 1
    if self.x >= ctx.width then
        self.x = 0
        self.y = self.y + 1
    end
    
    return false -- not done yet
end

function Generator:reset()
    self.x = 0
    self.y = 0
end

return Generator
