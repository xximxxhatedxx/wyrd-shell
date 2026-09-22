-- =============================================================================
-- TOKYO NIGHT DESIGN SYSTEM: DESIGN TOKENS
-- =============================================================================
-- A clean, dark theme that celebrates the lights of Downtown Tokyo
-- =============================================================================

local M = {}

M.colors = {
    -- Backgrounds & Surfaces (Night)
    bg_deep        = "#16161e", -- Night deep
    bg_panel       = "rgba(26, 27, 38, 0.90)", -- #1a1b26
    surface_1      = "rgba(36, 40, 59, 0.80)", -- #24283b
    surface_2      = "rgba(41, 46, 66, 0.70)", -- #292e42
    surface_3      = "rgba(59, 66, 97, 0.60)", -- #3b4261
    glass          = "rgba(192, 202, 245, 0.04)",
    glass_strong   = "rgba(192, 202, 245, 0.08)",
    backdrop       = "rgba(22, 22, 30, 0.70)",

    -- Accents & Glows
    primary        = "#7aa2f7", -- Tokyo Blue
    primary_glow   = "rgba(122, 162, 247, 0.35)",
    secondary      = "#bb9af7", -- Tokyo Magenta / Purple
    secondary_glow = "rgba(187, 154, 247, 0.30)",
    highlight      = "#e0af68", -- Yellow
    success        = "#9ece6a", -- Green
    warning        = "#ff9e64", -- Orange
    error          = "#f7768e", -- Red
    cyan           = "#7dcfff", -- Cyan
    teal           = "#1abc9c", -- Teal

    -- Typography
    text_primary   = "#c0caf5", -- Text
    text_secondary = "#a9b1d6", -- Subtext
    text_muted     = "#565f89", -- Comment Gray
    text_inverse   = "#1a1b26", -- Dark

    -- Borders
    border_subtle  = "rgba(122, 162, 247, 0.12)",
    border_glow    = "rgba(122, 162, 247, 0.35)",
}

M.gradients = {
    bar      = "linear-gradient(180deg, rgba(26, 27, 38, 0.92) 0%, rgba(22, 22, 30, 0.82) 100%)",
    surface  = "linear-gradient(135deg, rgba(36, 40, 59, 0.80) 0%, rgba(41, 46, 66, 0.60) 100%)",
    primary  = "linear-gradient(135deg, #7aa2f7 0%, #bb9af7 100%)",
    glow_bar = "linear-gradient(90deg, transparent 0%, rgba(122, 162, 247, 0.10) 50%, transparent 100%)",
}

M.typography = {
    font = "JetBrains Mono Nerd Font, MesloLGS Nerd Font, sans-serif",
}

M.shadows = {
    bar   = { radius = 32, opacity = 0.50, offset_x = 0, offset_y = 0, color = "#000000" },
    popup = { radius = 40, opacity = 0.55, offset_x = 0, offset_y = 12, color = "#000000" },
    glow  = { radius = 16, opacity = 0.30, color = "#7aa2f7" },
}

return M
