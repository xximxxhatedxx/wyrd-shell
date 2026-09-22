-- =============================================================================
-- NORD DESIGN SYSTEM: DESIGN TOKENS
-- =============================================================================
-- An arctic, north-bluish clean dark palette
-- =============================================================================

local M = {}

M.colors = {
    -- Backgrounds & Surfaces (Polar Night)
    bg_deep        = "#242933", -- nord0 deep
    bg_panel       = "rgba(46, 52, 64, 0.90)", -- nord0 with acrylic blur
    surface_1      = "rgba(59, 66, 82, 0.80)", -- nord1
    surface_2      = "rgba(67, 76, 94, 0.70)", -- nord2
    surface_3      = "rgba(76, 86, 106, 0.60)", -- nord3
    glass          = "rgba(236, 239, 244, 0.04)",
    glass_strong   = "rgba(236, 239, 244, 0.08)",
    backdrop       = "rgba(36, 41, 51, 0.70)",

    -- Accents & Glows (Frost & Aurora)
    primary        = "#88C0D0", -- Frost Ice Blue
    primary_glow   = "rgba(136, 192, 208, 0.35)",
    secondary      = "#81A1C1", -- Frost Dark Blue
    secondary_glow = "rgba(129, 161, 193, 0.30)",
    highlight      = "#EBCB8B", -- Aurora Yellow
    success        = "#A3BE8C", -- Aurora Green
    warning        = "#D08770", -- Aurora Orange
    error          = "#BF616A", -- Aurora Red

    -- Typography (Snow Storm)
    text_primary   = "#ECEFF4", -- nord6
    text_secondary = "#D8DEE9", -- nord4
    text_muted     = "#4C566A", -- nord3
    text_inverse   = "#2E3440", -- nord0

    -- Borders
    border_subtle  = "rgba(136, 192, 208, 0.12)",
    border_glow    = "rgba(136, 192, 208, 0.35)",
}

M.gradients = {
    bar      = "linear-gradient(180deg, rgba(46, 52, 64, 0.92) 0%, rgba(36, 41, 51, 0.82) 100%)",
    surface  = "linear-gradient(135deg, rgba(59, 66, 82, 0.80) 0%, rgba(67, 76, 94, 0.60) 100%)",
    primary  = "linear-gradient(135deg, #88C0D0 0%, #81A1C1 100%)",
    glow_bar = "linear-gradient(90deg, transparent 0%, rgba(136, 192, 208, 0.10) 50%, transparent 100%)",
}

M.typography = {
    font = "JetBrains Mono Nerd Font, MesloLGS Nerd Font, sans-serif",
}

M.shadows = {
    bar   = { radius = 32, opacity = 0.50, offset_x = 0, offset_y = 0, color = "#000000" },
    popup = { radius = 40, opacity = 0.55, offset_x = 0, offset_y = 12, color = "#000000" },
    glow  = { radius = 16, opacity = 0.30, color = "#88C0D0" },
}

return M
