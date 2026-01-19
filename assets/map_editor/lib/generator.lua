-- Generator base class for composable generation.
--
-- Follows PyTorch nn.Module pattern:
-- - Generators register children via add_child()
-- - Scene tree paths track location (e.g., "root.step_1.markov")
-- - Step info is emitted with path for visualizer filtering
--
-- Usage:
--   local Generator = require("lib.generator")
--   local MyGen = Generator:extend("MyGen")
--   
--   function MyGen:init(ctx)
--       self.child = self:add_child("child", some_generator)
--   end
--   
--   function MyGen:step(ctx)
--       return self.child:step(ctx)
--   end

local Generator = {}
Generator.__index = Generator

--- Create a new Generator instance.
-- @param type_name string The type name for this generator (e.g., "Sequential")
-- @return Generator
function Generator:new(type_name)
    local instance = setmetatable({}, self)
    instance._type = type_name or "Generator"
    instance._path = "root"
    instance._ctx = nil
    instance._children = {}
    instance._children_order = {}  -- Preserve insertion order
    instance._done = false
    instance._step_count = 0
    return instance
end

--- Extend Generator to create a subclass.
-- @param type_name string The type name for the subclass
-- @return table A new class that inherits from Generator
function Generator:extend(type_name)
    local cls = {}
    cls.__index = cls
    setmetatable(cls, { __index = self })
    cls._type = type_name or "Generator"
    
    -- Override new to set the correct type
    function cls:new(...)
        local instance = Generator.new(self, type_name)
        return instance
    end
    
    return cls
end

--- Set the scene tree path (called by parent when adding as child).
-- @param path string The full path (e.g., "root.step_1")
function Generator:_set_path(path)
    self._path = path
    -- Propagate to children
    for name, child in pairs(self._children) do
        if child._set_path then
            child:_set_path(path .. "." .. name)
        end
    end
end

--- Set the generator context (called by parent or runtime).
-- @param ctx table The GeneratorContext from Rust
function Generator:_set_context(ctx)
    self._ctx = ctx
    -- Propagate to children
    for _, child in pairs(self._children) do
        if child._set_context then
            child:_set_context(ctx)
        end
    end
end

--- Add a child generator.
-- @param name string The name for this child (used in path)
-- @param child table The child generator (must support _set_path, _set_context)
-- @return table The child (for chaining)
function Generator:add_child(name, child)
    local child_path = self._path .. "." .. name
    
    -- Set path on child
    if child._set_path then
        child:_set_path(child_path)
    elseif type(child) == "table" then
        child._path = child_path
    end
    
    -- Set context on child if we have one
    if self._ctx and child._set_context then
        child:_set_context(self._ctx)
    end
    
    self._children[name] = child
    table.insert(self._children_order, name)
    
    return child
end

--- Get a child by name.
-- @param name string The child name
-- @return table|nil The child or nil
function Generator:get_child(name)
    return self._children[name]
end

--- Emit step info tagged with this generator's path.
-- @param info table Step info fields (x, y, material_id, etc.)
function Generator:emit_step(info)
    if self._ctx and self._ctx.emit_step then
        info.path = self._path
        info.step_number = self._step_count
        self._ctx:emit_step(self._path, info)
    end
    self._step_count = self._step_count + 1
end

--- Get the recursive structure of this generator and its children.
-- @return table Structure with type, path, children
function Generator:get_structure()
    local children = {}
    for _, name in ipairs(self._children_order) do
        local child = self._children[name]
        if child.get_structure then
            children[name] = child:get_structure()
        else
            -- Leaf node (like MjModel) - get basic info
            children[name] = {
                type = child._type or "unknown",
                path = child._path or (self._path .. "." .. name),
            }
        end
    end
    
    local structure = {
        type = self._type,
        path = self._path,
    }
    
    -- Only include children if non-empty
    if next(children) then
        structure.children = children
    end
    
    return structure
end

--- Initialize the generator (override in subclass).
-- @param ctx table The GeneratorContext
function Generator:init(ctx)
    -- Default: do nothing
end

--- Execute one step of generation (override in subclass).
-- @param ctx table The GeneratorContext
-- @return boolean True if generation is complete
function Generator:step(ctx)
    return true  -- Default: immediately complete
end

--- Reset the generator state (override in subclass).
function Generator:reset()
    self._done = false
    self._step_count = 0
    for _, child in pairs(self._children) do
        if child.reset then
            -- MjModel.reset requires a seed argument
            if child._type == "MjModel" then
                child:reset(os.time())
            else
                child:reset()
            end
        end
    end
end

--- Check if generation is complete.
-- @return boolean
function Generator:is_done()
    return self._done
end

--- Mark generation as complete.
function Generator:complete()
    self._done = true
end

return Generator
