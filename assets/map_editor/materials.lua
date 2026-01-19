-- Map Editor Material Definitions
-- 
-- Materials can bind to MarkovJunior palette characters via mj_char:
--   mj_char = "B"  →  Use this material when MJ outputs 'B' (Black)
--   mj_char = "W"  →  Use this material when MJ outputs 'W' (White)
--
-- If an MJ model uses a character without a binding, the system
-- auto-creates a material using the color from MJ's palette.xml.
--
-- You can override MJ colors by specifying your own color with mj_char.
-- Or omit mj_char entirely for custom materials unrelated to MJ.

return {
    -- MJ palette bindings (using MJ default colors from palette.xml)
    { id = 1, name = "mj_black",  color = {0.0, 0.0, 0.0},       mj_char = "B", tags = {"mj"} },
    { id = 2, name = "mj_white",  color = {1.0, 0.945, 0.910},   mj_char = "W", tags = {"mj"} },
    { id = 3, name = "mj_gray",   color = {0.761, 0.765, 0.780}, mj_char = "A", tags = {"mj"} },
    
    -- Custom materials (no MJ binding)
    { id = 10, name = "crystal",  color = {0.0, 0.894, 0.212}, tags = {"decoration"} },
    { id = 11, name = "stone",    color = {0.5, 0.5, 0.5},     tags = {"natural", "terrain"} },
    { id = 12, name = "dirt",     color = {0.6, 0.4, 0.2},     tags = {"natural", "terrain"} },
    
    -- Example: Override MJ colors with custom materials
    -- { id = 20, name = "stone_wall",  color = {0.3, 0.3, 0.35}, mj_char = "B", tags = {"custom"} },
    -- { id = 21, name = "stone_floor", color = {0.5, 0.5, 0.5},  mj_char = "W", tags = {"custom"} },
}
