-- =============================================================================
-- NORD DESIGN SYSTEM: BAR STYLES
-- =============================================================================

local M = {}

function M.apply(tokens)
    local colors = tokens.colors
    local gradients = tokens.gradients
    local font = tokens.typography.font
    local shadows = tokens.shadows

    wyrd.style("global", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_primary,
        accent     = colors.primary,
        font       = font,
        font_size  = 11,
    })

    -- Floating Capsule Bar
    wyrd.style("bar", {
        background = gradients.bar,
        foreground = colors.text_primary,
        accent     = colors.primary,
        outline    = "1px solid " .. colors.border_subtle,
        shadow     = shadows.bar,
        radius     = 9999,
        padding    = { horizontal = 16, vertical = 4 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("bar_slot", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_secondary,
        padding    = { horizontal = 4, vertical = 2 },
    })

    -- Glass Capsule Pill (default module chip)
    wyrd.style("chip", {
        background = colors.glass,
        foreground = colors.text_secondary,
        accent     = colors.primary,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 9999,
        padding    = { horizontal = 14, vertical = 4 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.border_glow,
            shadow     = { radius = 12, opacity = 0.15, color = colors.primary },
        },
        active     = {
            background = colors.surface_3,
            foreground = colors.primary,
            outline    = "1px solid " .. colors.primary,
        },
    })

    -- Accent chip for active states
    wyrd.style("chip_accent", {
        background = colors.surface_2,
        foreground = colors.primary,
        accent     = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 9999,
        padding    = { horizontal = 14, vertical = 4 },
        font       = font,
        font_size  = 11,
        shadow     = { radius = 8, opacity = 0.20, color = colors.primary },
        hover      = {
            background = colors.surface_3,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.primary,
        },
    })

    wyrd.style("clean_item", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_secondary,
        radius     = 9999,
        padding    = { horizontal = 8, vertical = 4 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.glass_strong,
            foreground = colors.text_primary,
        },
    })

    -- Launcher Round Button
    wyrd.style("launcher_round", {
        background = colors.surface_2,
        foreground = colors.primary,
        accent     = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 9999,
        font       = font,
        font_size  = 14,
        shadow     = { radius = 10, opacity = 0.25, color = colors.primary },
        hover      = {
            background = colors.surface_3,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.primary,
            shadow     = { radius = 14, opacity = 0.35, color = colors.primary },
        },
        active     = {
            background = colors.primary,
            foreground = colors.text_inverse,
        },
    })

    -- Power Round Button
    wyrd.style("power_round", {
        background = colors.glass,
        foreground = colors.error,
        outline    = "1px solid rgba(191, 97, 106, 0.20)",
        radius     = 9999,
        font       = font,
        font_size  = 12,
        hover      = {
            background = colors.error,
            foreground = colors.text_inverse,
            outline    = "1px solid " .. colors.error,
            shadow     = { radius = 12, opacity = 0.40, color = colors.error },
        },
    })

    -- Workspace indicators
    wyrd.style("ws_active", {
        background = colors.primary,
        foreground = colors.text_inverse,
        radius     = 9999,
        font       = font,
        font_size  = 10,
        shadow     = { radius = 8, opacity = 0.30, color = colors.primary },
    })

    wyrd.style("ws_occupied", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 9999,
        font       = font,
        font_size  = 10,
    })

    wyrd.style("ws_empty", {
        background = colors.glass,
        foreground = colors.text_muted,
        radius     = 9999,
        font       = font,
        font_size  = 10,
    })

    wyrd.style("ws_urgent", {
        background = colors.error,
        foreground = colors.text_inverse,
        radius     = 9999,
        font       = font,
        font_size  = 10,
        shadow     = { radius = 10, opacity = 0.40, color = colors.error },
    })
end

return M
