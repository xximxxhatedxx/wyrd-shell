-- =============================================================================
-- AETHERIA DESIGN SYSTEM: DESIGN TOKENS
-- =============================================================================
-- Deep cosmic acrylic with bioluminescent accents
-- =============================================================================

local M = {}

M.colors = {
    -- Backgrounds & Surfaces
    bg_deep        = "#050508",
    bg_panel       = "rgba(12, 14, 22, 0.82)",
    surface_1      = "rgba(22, 26, 38, 0.72)",
    surface_2      = "rgba(32, 38, 56, 0.58)",
    surface_3      = "rgba(45, 52, 72, 0.45)",
    glass          = "rgba(255, 255, 255, 0.03)",
    glass_strong   = "rgba(255, 255, 255, 0.07)",
    backdrop       = "rgba(5, 5, 8, 0.65)",

    -- Accents & Glows
    primary        = "#7DD3FC", -- Sky Blue
    primary_glow   = "rgba(125, 211, 252, 0.35)",
    secondary      = "#C4B5FD", -- Lavender
    secondary_glow = "rgba(196, 181, 253, 0.30)",
    highlight      = "#FDE047", -- Soft Yellow
    success        = "#6EE7B7", -- Emerald
    warning        = "#FDBA74", -- Peach
    error          = "#FCA5A5", -- Soft Rose Red

    -- Typography
    text_primary   = "#E2E8F0",
    text_secondary = "#94A3B8",
    text_muted     = "#64748B",
    text_inverse   = "#0F172A",

    -- Borders
    border_subtle  = "rgba(125, 211, 252, 0.08)",
    border_glow    = "rgba(125, 211, 252, 0.25)",
}

M.gradients = {
    bar      = "linear-gradient(180deg, rgba(12, 14, 22, 0.88) 0%, rgba(12, 14, 22, 0.72) 100%)",
    surface  = "linear-gradient(135deg, rgba(22, 26, 38, 0.75) 0%, rgba(32, 38, 56, 0.55) 100%)",
    primary  = "linear-gradient(135deg, #7DD3FC 0%, #C4B5FD 100%)",
    glow_bar = "linear-gradient(90deg, transparent 0%, rgba(125, 211, 252, 0.08) 50%, transparent 100%)",
}

M.typography = {
    font = "JetBrains Mono Nerd Font, MesloLGS Nerd Font, sans-serif",
}

M.shadows = {
    bar   = { radius = 32, opacity = 0.50, offset_x = 0, offset_y = 0, color = "#000000" },
    popup = { radius = 40, opacity = 0.55, offset_x = 0, offset_y = 12, color = "#000000" },
    glow  = { radius = 16, opacity = 0.30, color = "#7DD3FC" },
}

return M
