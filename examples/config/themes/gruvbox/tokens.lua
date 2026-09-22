-- =============================================================================
-- GRUVBOX DARK DESIGN SYSTEM: DESIGN TOKENS
-- =============================================================================
-- Retro groove warm dark palette with iconic earthy tones
-- =============================================================================

local M = {}

M.colors = {
    -- Backgrounds & Surfaces (Gruvbox Dark)
    bg_deep        = "#1d2021", -- dark0_hard
    bg_panel       = "rgba(40, 40, 40, 0.92)", -- dark0
    surface_1      = "rgba(60, 56, 54, 0.80)", -- dark1
    surface_2      = "rgba(80, 73, 69, 0.70)", -- dark2
    surface_3      = "rgba(102, 92, 84, 0.60)", -- dark3
    glass          = "rgba(235, 219, 178, 0.04)",
    glass_strong   = "rgba(235, 219, 178, 0.08)",
    backdrop       = "rgba(29, 32, 33, 0.75)",

    -- Accents & Highlights
    primary        = "#fe8019", -- Bright Orange
    primary_glow   = "rgba(254, 128, 25, 0.35)",
    secondary      = "#fabd2f", -- Bright Yellow
    secondary_glow = "rgba(250, 189, 47, 0.30)",
    highlight      = "#fabd2f", -- Yellow
    success        = "#b8bb26", -- Bright Green
    warning        = "#fe8019", -- Bright Orange
    error          = "#fb4934", -- Bright Red
    blue           = "#83a598", -- Bright Blue
    purple         = "#d3869b", -- Bright Purple
    aqua           = "#8ec07c", -- Bright Aqua

    -- Typography
    text_primary   = "#ebdbb2", -- Light 1
    text_secondary = "#d5c4a1", -- Light 2
    text_muted     = "#928374", -- Gray
    text_inverse   = "#282828", -- Dark 0

    -- Borders
    border_subtle  = "rgba(254, 128, 25, 0.12)",
    border_glow    = "rgba(254, 128, 25, 0.35)",
}

M.gradients = {
    bar      = "linear-gradient(180deg, rgba(40, 40, 40, 0.94) 0%, rgba(29, 32, 33, 0.84) 100%)",
    surface  = "linear-gradient(135deg, rgba(60, 56, 54, 0.85) 0%, rgba(80, 73, 69, 0.65) 100%)",
    primary  = "linear-gradient(135deg, #fe8019 0%, #fabd2f 100%)",
    glow_bar = "linear-gradient(90deg, transparent 0%, rgba(254, 128, 25, 0.10) 50%, transparent 100%)",
}

M.typography = {
    font = "JetBrains Mono Nerd Font, MesloLGS Nerd Font, sans-serif",
}

M.shadows = {
    bar   = { radius = 32, opacity = 0.50, offset_x = 0, offset_y = 0, color = "#000000" },
    popup = { radius = 40, opacity = 0.55, offset_x = 0, offset_y = 12, color = "#000000" },
    glow  = { radius = 16, opacity = 0.30, color = "#fe8019" },
}

return M
