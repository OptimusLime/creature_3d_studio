-- Map Editor Generator Script - Vertical Stripes
-- Used for testing hot reload

local Generator = {}

function Generator:init(ctx)
    self.x = 0
    self.y = 0
end

function Generator:step(ctx)
    if self.y >= ctx.height then
        return true -- done
    end
    
    -- Get materials from palette
    local mat_a = ctx.palette[1] or 1
    local mat_b = ctx.palette[2] or mat_a
    
    -- VERTICAL STRIPES pattern: alternate based on x only
    local mat = (self.x % 2 == 0) and mat_a or mat_b
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
