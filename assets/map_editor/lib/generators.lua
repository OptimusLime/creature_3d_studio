-- Built-in generator composers and utilities.
--
-- Provides:
-- - generators.sequential({...}) - Run generators in order
-- - generators.parallel({...}) - Run all generators each step
-- - generators.scatter({...}) - Scatter material on target cells
--
-- Usage:
--   local generators = require("lib.generators")
--   
--   return generators.sequential({
--       mj.load_model("MazeGrowth.xml"),
--       generators.scatter({ material = 3, target = 1, density = 0.05 })
--   })

local Generator = require("lib.generator")

local generators = {}

--------------------------------------------------------------------------------
-- Sequential: Run generators in order, one at a time
--------------------------------------------------------------------------------

local Sequential = Generator:extend("Sequential")

function Sequential:new(children)
    local instance = Generator.new(self, "Sequential")
    instance._current_index = 1
    instance._child_names = {}
    
    -- Add children with auto-generated names
    for i, child in ipairs(children) do
        local name = "step_" .. i
        instance:add_child(name, child)
        table.insert(instance._child_names, name)
    end
    
    return instance
end

function Sequential:init(ctx)
    self:_set_context(ctx)
    self._current_index = 1
    self._done = false
    
    -- Initialize all children
    for _, name in ipairs(self._child_names) do
        local child = self._children[name]
        if child.init then
            child:init(ctx)
        end
    end
end

function Sequential:step(ctx)
    -- Check if we've completed all children
    if self._current_index > #self._child_names then
        self._done = true
        return true
    end
    
    local name = self._child_names[self._current_index]
    local child = self._children[name]
    
    -- Step the current child
    local child_done = false
    if child.step then
        child_done = child:step(ctx)
    elseif child.is_done then
        child_done = child:is_done()
    else
        child_done = true
    end
    
    -- If current child is done, move to next
    if child_done then
        self._current_index = self._current_index + 1
        
        -- Check if all done
        if self._current_index > #self._child_names then
            self._done = true
            return true
        end
    end
    
    return false
end

function Sequential:reset()
    Generator.reset(self)
    self._current_index = 1
end

function Sequential:get_structure()
    local base = Generator.get_structure(self)
    base.current_index = self._current_index
    return base
end

generators.sequential = function(children)
    return Sequential:new(children)
end

--------------------------------------------------------------------------------
-- Parallel: Run all generators each step, complete when all are done
--------------------------------------------------------------------------------

local Parallel = Generator:extend("Parallel")

function Parallel:new(children)
    local instance = Generator.new(self, "Parallel")
    instance._child_names = {}
    
    -- Add children with auto-generated names
    for i, child in ipairs(children) do
        local name = "branch_" .. i
        instance:add_child(name, child)
        table.insert(instance._child_names, name)
    end
    
    return instance
end

function Parallel:init(ctx)
    self:_set_context(ctx)
    self._done = false
    
    -- Initialize all children
    for _, name in ipairs(self._child_names) do
        local child = self._children[name]
        if child.init then
            child:init(ctx)
        end
    end
end

function Parallel:step(ctx)
    local all_done = true
    
    for _, name in ipairs(self._child_names) do
        local child = self._children[name]
        
        -- Check if child is already done
        local child_done = false
        if child.is_done then
            child_done = child:is_done()
        end
        
        -- Step if not done
        if not child_done then
            if child.step then
                child_done = child:step(ctx)
            end
        end
        
        if not child_done then
            all_done = false
        end
    end
    
    self._done = all_done
    return all_done
end

generators.parallel = function(children)
    return Parallel:new(children)
end

--------------------------------------------------------------------------------
-- Scatter: Place material randomly on cells matching target
--------------------------------------------------------------------------------

local Scatter = Generator:extend("Scatter")

function Scatter:new(opts)
    local instance = Generator.new(self, "Scatter")
    instance._material = opts.material or 3
    instance._target = opts.target or 1
    instance._density = opts.density or 0.1
    instance._seed = opts.seed or os.time()
    instance._x = 0
    instance._y = 0
    instance._rng_state = instance._seed
    return instance
end

-- Simple PRNG (xorshift)
function Scatter:_random()
    local x = self._rng_state
    x = x ~ (x << 13)
    x = x ~ (x >> 17)
    x = x ~ (x << 5)
    -- Handle Lua's number representation
    x = x % 2147483647
    if x < 0 then x = x + 2147483647 end
    self._rng_state = x
    return x / 2147483647
end

function Scatter:init(ctx)
    self:_set_context(ctx)
    self._x = 0
    self._y = 0
    self._done = false
    self._rng_state = self._seed
end

function Scatter:step(ctx)
    -- Scan through grid looking for target material
    while self._y < ctx.height do
        while self._x < ctx.width do
            local current = ctx:get_voxel(self._x, self._y)
            
            if current == self._target then
                -- Roll for scatter
                if self:_random() < self._density then
                    ctx:set_voxel(self._x, self._y, self._material)
                    
                    -- Emit step info
                    self:emit_step({
                        x = self._x,
                        y = self._y,
                        material_id = self._material,
                        completed = false,
                    })
                end
            end
            
            self._x = self._x + 1
            
            -- Return after each cell to allow step-by-step visualization
            -- (This makes scatter slower but more visual)
            return false
        end
        
        self._x = 0
        self._y = self._y + 1
    end
    
    self._done = true
    return true
end

function Scatter:reset()
    Generator.reset(self)
    self._x = 0
    self._y = 0
    self._rng_state = self._seed
end

function Scatter:get_structure()
    local base = Generator.get_structure(self)
    base.material = self._material
    base.target = self._target
    base.density = self._density
    return base
end

generators.scatter = function(opts)
    return Scatter:new(opts)
end

--------------------------------------------------------------------------------
-- Fill: Fill cells matching a condition
--------------------------------------------------------------------------------

local Fill = Generator:extend("Fill")

function Fill:new(opts)
    local instance = Generator.new(self, "Fill")
    instance._material = opts.material or 1
    instance._where = opts.where or "all"  -- "all", "empty", "border"
    instance._x = 0
    instance._y = 0
    return instance
end

function Fill:init(ctx)
    self:_set_context(ctx)
    self._x = 0
    self._y = 0
    self._done = false
end

function Fill:_matches(ctx, x, y)
    local where = self._where
    
    if where == "all" then
        return true
    elseif where == "empty" then
        return ctx:get_voxel(x, y) == 0
    elseif where == "border" then
        return x == 0 or y == 0 or x == ctx.width - 1 or y == ctx.height - 1
    else
        return false
    end
end

function Fill:step(ctx)
    while self._y < ctx.height do
        while self._x < ctx.width do
            if self:_matches(ctx, self._x, self._y) then
                ctx:set_voxel(self._x, self._y, self._material)
                
                self:emit_step({
                    x = self._x,
                    y = self._y,
                    material_id = self._material,
                    completed = false,
                })
            end
            
            self._x = self._x + 1
            return false  -- One cell per step
        end
        
        self._x = 0
        self._y = self._y + 1
    end
    
    self._done = true
    return true
end

function Fill:reset()
    Generator.reset(self)
    self._x = 0
    self._y = 0
end

generators.fill = function(opts)
    return Fill:new(opts)
end

return generators
