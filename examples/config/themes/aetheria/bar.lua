-- =============================================================================
-- AETHERIA DESIGN SYSTEM: BAR STYLES
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

    -- Active Accent Chip
    wyrd.style("chip_accent", {
        background = "linear-gradient(135deg, rgba(125, 211, 252, 0.20) 0%, rgba(196, 181, 253, 0.15) 100%)",
        foreground = colors.primary,
        accent     = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 9999,
        padding    = { horizontal = 14, vertical = 4 },
        font       = font,
        font_size  = 11,
        shadow     = { radius = 12, opacity = 0.20, color = colors.primary },
        hover      = {
            background = "linear-gradient(135deg, rgba(125, 211, 252, 0.30) 0%, rgba(196, 181, 253, 0.25) 100%)",
            foreground = "#FFFFFF",
            outline    = "1px solid " .. colors.primary,
        },
    })

    -- Launcher Round Button (36x32 pill)
    wyrd.style("launcher_round", {
        background = "linear-gradient(135deg, rgba(125, 211, 252, 0.22) 0%, rgba(196, 181, 253, 0.15) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 9999,
        font       = font,
        font_size  = 14,
        shadow     = { radius = 14, opacity = 0.25, color = colors.primary },
        hover      = {
            background = "linear-gradient(135deg, rgba(125, 211, 252, 0.35) 0%, rgba(196, 181, 253, 0.25) 100%)",
            foreground = "#FFFFFF",
            outline    = "1px solid " .. colors.primary,
            shadow     = { radius = 18, opacity = 0.40, color = colors.primary },
        },
    })

    -- Power Round Button (32x32 pill)
    wyrd.style("power_round", {
        background = colors.glass,
        foreground = colors.error,
        outline    = "1px solid rgba(252, 165, 165, 0.15)",
        radius     = 9999,
        font       = font,
        font_size  = 12,
        hover      = {
            background = "rgba(252, 165, 165, 0.18)",
            foreground = "#FFFFFF",
            outline    = "1px solid " .. colors.error,
            shadow     = { radius = 14, opacity = 0.35, color = colors.error },
        },
    })

    -- System Tray Items
    wyrd.style("clean_item", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_secondary,
        padding    = { horizontal = 4, vertical = 2 },
        hover      = {
            foreground = colors.text_primary,
        },
    })

    wyrd.style("tray_item", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_secondary,
        font       = font,
        font_size  = 13,
        padding    = { horizontal = 4, vertical = 2 },
        hover      = {
            foreground = colors.primary,
        },
    })

    wyrd.style("clean_accent", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.primary,
        font_weight = "bold",
    })

    -- Typography styles
    wyrd.style("title_bold", {
        foreground = colors.text_primary,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("secondary", {
        foreground = colors.text_secondary,
        font_size  = 11,
    })

    wyrd.style("muted", {
        foreground = colors.text_muted,
        font_size  = 10,
    })

    wyrd.style("highlight", {
        foreground = colors.highlight,
        font_size  = 11,
    })
end

return M
