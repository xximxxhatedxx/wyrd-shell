-- =============================================================================
-- WYRD SHELL: MAIN CONFIGURATION
-- =============================================================================

-- 1. Load User Settings
local settings = require("settings").load()

-- 2. Apply Theme Styles
local theme_name = settings.theme or "catppuccin"
local ok, theme_mod = pcall(require, "themes." .. theme_name)
if ok and theme_mod and type(theme_mod.apply) == "function" then
    theme_mod.apply()
elseif settings.theme == "aetheria" then
    require("themes.aetheria").apply()
else
    require("themes.catppuccin").apply()
end

-- 3. Load Core System Modules
Module.load("audio")
Module.load("battery")
Module.load("bluetooth")
Module.load("brightness")
Module.load("clipboard")
Module.load("clock")
Module.load("dnd")
Module.load("dynamic-color")
Module.load("keyboard")
Module.load("launcher")
Module.load("mpris")
Module.load("network")
Module.load("notifications")
Module.load("power-menu")
Module.load("system")
Module.load("tray")
Module.load("window")
Module.load("workspaces")

-- 4. Create Declarative Surfaces
if settings.layout == "left" then
    require("surfaces.bar_left")
else
    require("surfaces.bar")
end

require("surfaces.popups")
