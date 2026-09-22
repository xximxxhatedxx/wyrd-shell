-- =============================================================================
-- CATPPUCCIN MOCHA DESIGN SYSTEM: DESIGN TOKENS
-- =============================================================================
-- Soothing pastel dark palette for the high-spirited Wayland desktop
-- =============================================================================

local M = {}

M.colors = {
    -- Backgrounds & Acrylic Surfaces
    bg_deep        = "#11111b", -- Crust
    bg_panel       = "rgba(30, 30, 46, 0.88)", -- Base with acrylic blur
    surface_1      = "rgba(49, 50, 68, 0.75)", -- Surface0
    surface_2      = "rgba(69, 71, 90, 0.65)", -- Surface1
    surface_3      = "rgba(88, 91, 112, 0.55)", -- Surface2
    glass          = "rgba(205, 214, 244, 0.04)", -- Subtle text tint
    glass_strong   = "rgba(205, 214, 244, 0.08)",
    backdrop       = "rgba(17, 17, 27, 0.70)", -- Dimmed overlay

    -- Catppuccin Accents & Glows
    primary        = "#cba6f7", -- Mauve
    primary_glow   = "rgba(203, 166, 247, 0.35)",
    secondary      = "#b4befe", -- Lavender
    secondary_glow = "rgba(180, 190, 254, 0.30)",
    highlight      = "#f9e2af", -- Yellow
    success        = "#a6e3a1", -- Green
    warning        = "#fab387", -- Peach
    error          = "#f38ba8", -- Red
    blue           = "#89b4fa", -- Blue
    sapphire       = "#74c7ec", -- Sapphire
    teal           = "#94e2d5", -- Teal
    pink           = "#f5c2e7", -- Pink

    -- Typography
    text_primary   = "#cdd6f4", -- Text
    text_secondary = "#a6adc8", -- Subtext0
    text_muted     = "#6c7086", -- Overlay0
    text_inverse   = "#11111b", -- Crust

    -- Borders
    border_subtle  = "rgba(203, 166, 247, 0.12)",
    border_glow    = "rgba(203, 166, 247, 0.35)",
}

M.gradients = {
    bar      = "linear-gradient(180deg, rgba(30, 30, 46, 0.92) 0%, rgba(24, 24, 37, 0.82) 100%)",
    surface  = "linear-gradient(135deg, rgba(49, 50, 68, 0.75) 0%, rgba(69, 71, 90, 0.55) 100%)",
    primary  = "linear-gradient(135deg, #cba6f7 0%, #b4befe 100%)",
    glow_bar = "linear-gradient(90deg, transparent 0%, rgba(203, 166, 247, 0.10) 50%, transparent 100%)",
}

M.typography = {
    font = "JetBrains Mono Nerd Font, MesloLGS Nerd Font, sans-serif",
}

M.shadows = {
    bar   = { radius = 32, opacity = 0.50, offset_x = 0, offset_y = 0, color = "#000000" },
    popup = { radius = 40, opacity = 0.55, offset_x = 0, offset_y = 12, color = "#000000" },
    glow  = { radius = 16, opacity = 0.30, color = "#cba6f7" },
}

return M
