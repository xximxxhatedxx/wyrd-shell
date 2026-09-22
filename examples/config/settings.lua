-- =============================================================================
-- WYRD SHELL: SETTINGS LOADER
-- =============================================================================
-- Reads ~/.config/wyrd/settings.toml or falls back to system defaults.
-- =============================================================================

local M = {}

function M.load()
    -- Check if wyrd.settings was injected by the core runtime
    if wyrd and wyrd.settings and type(wyrd.settings) == "table" then
        return wyrd.settings
    end

    local defaults = {
        theme = "catppuccin", -- "catppuccin" or "aetheria"
        layout = "top",       -- "top" or "left"
        dynamic_color = true, -- adapt to wallpaper colors
    }

    local home = os.getenv("HOME") or ""
    local config_dir = os.getenv("XDG_CONFIG_HOME") or (home .. "/.config")
    local file_path = config_dir .. "/wyrd/settings.toml"

    local f = io.open(file_path, "r")
    if not f then
        return defaults
    end

    local content = f:read("*all")
    f:close()

    local settings = {}
    for line in content:gmatch("[^\r\n]+") do
        line = line:match("^%s*(.-)%s*$")
        if not line:match("^#") and not line:match("^%[") then
            local k, v = line:match('^([%w_]+)%s*=%s*"([^"]+)"')
            if k and v then
                settings[k] = v
            else
                local kb, vb = line:match('^([%w_]+)%s*=%s*(%a+)')
                if kb and vb then
                    settings[kb] = (vb == "true")
                end
            end
        end
    end

    for k, v in pairs(defaults) do
        if settings[k] == nil then
            settings[k] = v
        end
    end

    return settings
end

return M
