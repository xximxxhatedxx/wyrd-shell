-- =============================================================================
-- AETHERIA DESIGN SYSTEM: THEME INITIALIZATION
-- =============================================================================

local M = {}

M.tokens = require("themes.aetheria.tokens")
M.bar = require("themes.aetheria.bar")
M.popups = require("themes.aetheria.popups")

function M.apply()
    M.bar.apply(M.tokens)
    M.popups.apply(M.tokens)
end

function M.set_accent(hex)
    if not hex or hex == "" then return end
    M.tokens.colors.primary = hex
    M.tokens.colors.primary_glow = hex .. "59"
    M.tokens.colors.border_glow = hex .. "40"
    if M.tokens.shadows and M.tokens.shadows.glow then
        M.tokens.shadows.glow.color = hex
    end
    M.apply()
end

function M.set_palette(palette)
    if not palette or type(palette) ~= "table" then return end
    local primary = palette.primary or palette.accent
    if primary and primary ~= "" then
        M.tokens.colors.primary = primary
        M.tokens.colors.accent = primary
        M.tokens.colors.primary_glow = primary .. "59"
        M.tokens.colors.border_glow = primary .. "40"
        if M.tokens.shadows and M.tokens.shadows.glow then
            M.tokens.shadows.glow.color = primary
        end
    end
    if palette.secondary and palette.secondary ~= "" then
        M.tokens.colors.secondary = palette.secondary
        M.tokens.colors.secondary_glow = palette.secondary .. "40"
    end
    if palette.surface and palette.surface ~= "" then
        M.tokens.colors.surface_1 = palette.surface .. "d9"
    end
    if palette.surface_container and palette.surface_container ~= "" then
        M.tokens.colors.surface_2 = palette.surface_container .. "c0"
    end
    if palette.surface_variant and palette.surface_variant ~= "" then
        M.tokens.colors.surface_3 = palette.surface_variant .. "a6"
    end
    if palette.background and palette.background ~= "" then
        M.tokens.colors.bg_deep = palette.background
        M.tokens.colors.bg_panel = palette.background .. "d9"
    end
    if palette.on_surface and palette.on_surface ~= "" then
        M.tokens.colors.text_primary = palette.on_surface
    end
    if palette.on_surface_variant and palette.on_surface_variant ~= "" then
        M.tokens.colors.text_secondary = palette.on_surface_variant
    end
    local outline = palette.outline_variant or palette.outline
    if outline and outline ~= "" then
        M.tokens.colors.border_subtle = outline .. "40"
    end
    M.apply()
end

if _G.wyrd and type(_G.wyrd) == "table" then
    _G.wyrd._on_theme_accent = function(hex)
        M.set_accent(hex)
    end
    _G.wyrd._on_theme_palette = function(palette)
        M.set_palette(palette)
    end
end

return M
