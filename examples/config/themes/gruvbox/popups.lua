-- =============================================================================
-- GRUVBOX DARK DESIGN SYSTEM: POPUP STYLES
-- =============================================================================

local M = {}

function M.apply(tokens)
    local colors = tokens.colors
    local gradients = tokens.gradients
    local font = tokens.typography.font
    local shadows = tokens.shadows

    -- ============================================================
    -- 1. BASE POPUP CONTAINERS & CARDS
    -- ============================================================

    wyrd.style("popup", {
        background = colors.bg_panel,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_subtle,
        shadow     = shadows.popup,
        radius     = 24,
        padding    = { horizontal = 16, vertical = 16 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("card", {
        background = colors.surface_1,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            outline    = "1px solid " .. colors.border_glow,
        },
    })

    wyrd.style("card_elevated", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 16,
        padding    = { horizontal = 14, vertical = 12 },
        font       = font,
        font_size  = 11,
        shadow     = { radius = 18, opacity = 0.12, color = colors.primary },
    })

    wyrd.style("card_nested", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 12,
        padding    = { horizontal = 10, vertical = 8 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("card_title", {
        foreground = colors.text_primary,
        font_size  = 12,
        font_weight = "bold",
    })

    wyrd.style("section_title", {
        foreground = colors.secondary,
        font_size  = 10,
        font_weight = "bold",
    })

    wyrd.style("stat_hero", {
        foreground = colors.primary,
        font_size  = 20,
        font_weight = "bold",
    })

    wyrd.style("clock_large", {
        foreground = colors.text_primary,
        font_size  = 34,
        font_weight = "bold",
    })

    -- Generic Row Button
    wyrd.style("row_btn", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 10,
        padding    = { horizontal = 12, vertical = 8 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.border_subtle,
        },
    })

    wyrd.style("row_btn_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 10,
        padding    = { horizontal = 12, vertical = 8 },
        font       = font,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("slider_track", {
        foreground = colors.primary,
        accent     = colors.primary,
        background = colors.surface_3,
        radius     = 9999,
        padding    = { horizontal = 4, vertical = 4 },
    })

    -- ============================================================
    -- 2. AUDIO MODULE STYLES
    -- ============================================================

    wyrd.style("audio_banner", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 16,
        padding    = { horizontal = 14, vertical = 12 },
    })

    wyrd.style("audio_title", {
        foreground = colors.primary,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("audio_device_name", {
        foreground = colors.text_primary,
        font_size  = 12,
        font_weight = "bold",
    })

    wyrd.style("audio_mute_btn", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 9999,
        padding    = { horizontal = 12, vertical = 4 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    wyrd.style("audio_mute_active", {
        background = colors.error,
        foreground = colors.bg_deep,
        radius     = 9999,
        padding    = { horizontal = 12, vertical = 4 },
        font_weight = "bold",
    })

    wyrd.style("audio_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
    })

    wyrd.style("audio_card_title", {
        foreground = colors.secondary,
        font_size  = 10,
        font_weight = "bold",
    })

    wyrd.style("audio_slider", {
        foreground = colors.primary,
        accent     = colors.primary,
        background = colors.surface_3,
        radius     = 9999,
    })

    wyrd.style("audio_preset_btn", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 8, vertical = 4 },
        hover      = { background = colors.surface_2, foreground = colors.text_primary },
    })

    wyrd.style("audio_preset_active", {
        background = colors.primary,
        foreground = colors.bg_deep,
        radius     = 8,
        padding    = { horizontal = 8, vertical = 4 },
        font_weight = "bold",
        hover      = { background = colors.primary_glow, foreground = colors.bg_deep },
    })

    wyrd.style("audio_sink_btn", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary, outline = "1px solid " .. colors.border_glow },
    })

    wyrd.style("audio_sink_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font_weight = "bold",
        hover      = { background = "linear-gradient(135deg, rgba(254, 128, 25, 0.30) 0%, rgba(250, 189, 47, 0.22) 100%)", foreground = colors.primary },
    })

    wyrd.style("audio_source_btn", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary, outline = "1px solid " .. colors.border_glow },
    })

    wyrd.style("audio_source_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font_weight = "bold",
        hover      = { background = "linear-gradient(135deg, rgba(254, 128, 25, 0.30) 0%, rgba(250, 189, 47, 0.22) 100%)", foreground = colors.primary },
    })

    -- Aliases for device item styles
    wyrd.style("audio_device_item", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary, outline = "1px solid " .. colors.border_glow },
    })

    wyrd.style("audio_device_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font_weight = "bold",
        hover      = { background = "linear-gradient(135deg, rgba(254, 128, 25, 0.30) 0%, rgba(250, 189, 47, 0.22) 100%)", foreground = colors.primary },
    })

    -- ============================================================
    -- 3. NETWORK MODULE STYLES
    -- ============================================================

    wyrd.style("net_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
    })

    wyrd.style("net_banner", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 16,
        padding    = { horizontal = 14, vertical = 12 },
    })

    wyrd.style("net_title", {
        foreground = colors.primary,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("net_ssid", {
        foreground = colors.text_primary,
        font_size  = 14,
        font_weight = "bold",
    })

    wyrd.style("net_item", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    wyrd.style("net_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font_weight = "bold",
    })

    wyrd.style("net_btn", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_2, foreground = colors.text_primary },
    })

    wyrd.style("net_toggle", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 9999,
        padding    = { horizontal = 12, vertical = 4 },
        hover      = { background = colors.surface_3, outline = "1px solid " .. colors.border_glow },
    })

    -- ============================================================
    -- 4. BLUETOOTH MODULE STYLES
    -- ============================================================

    wyrd.style("bt_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
    })

    wyrd.style("bt_banner", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 16,
        padding    = { horizontal = 14, vertical = 12 },
    })

    wyrd.style("bt_title", {
        foreground = colors.primary,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("bt_item", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    wyrd.style("bt_active", {
        background = "linear-gradient(135deg, rgba(254, 128, 25, 0.20) 0%, rgba(250, 189, 47, 0.14) 100%)",
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font_weight = "bold",
    })

    wyrd.style("bt_btn", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_2, foreground = colors.text_primary },
    })

    wyrd.style("bt_toggle", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 9999,
        padding    = { horizontal = 12, vertical = 4 },
        hover      = { background = colors.surface_3, outline = "1px solid " .. colors.border_glow },
    })

    wyrd.style("bt_scan", {
        background = colors.surface_2,
        foreground = colors.secondary,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, outline = "1px solid " .. colors.border_glow },
    })

    -- ============================================================
    -- 5. NOTIFICATIONS MODULE STYLES
    -- ============================================================

    wyrd.style("notif_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 12,
        padding    = { horizontal = 12, vertical = 8 },
        hover      = { background = colors.surface_2, outline = "1px solid " .. colors.border_glow },
    })

    wyrd.style("notif_title", {
        foreground = colors.text_primary,
        font_size  = 12,
        font_weight = "bold",
    })

    wyrd.style("notif_body", {
        foreground = colors.text_secondary,
        font_size  = 11,
    })

    wyrd.style("notif_time", {
        foreground = colors.text_muted,
        font_size  = 10,
    })

    wyrd.style("notif_btn", {
        background = colors.glass,
        foreground = colors.text_muted,
        radius     = 6,
        padding    = { horizontal = 6, vertical = 2 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    wyrd.style("notif_dnd", {
        background = colors.surface_2,
        foreground = colors.warning,
        radius     = 9999,
        padding    = { horizontal = 12, vertical = 4 },
    })

    wyrd.style("notif_clear", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 8,
        padding    = { horizontal = 12, vertical = 6 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    wyrd.style("notif_empty", {
        foreground = colors.text_muted,
        font_size  = 12,
    })

    -- ============================================================
    -- 6. LAUNCHER MODULE STYLES
    -- ============================================================

    wyrd.style("launcher_search", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 10, vertical = 6 },
    })

    wyrd.style("launcher_input", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_primary,
        accent     = colors.primary,
        font       = font,
        font_size  = 13,
    })

    wyrd.style("launcher_input_icon", {
        foreground = colors.primary,
        font_size  = 14,
    })

    wyrd.style("launcher_item", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = {
            background = colors.surface_3,
            foreground = colors.primary,
            outline    = "1px solid " .. colors.border_glow,
        },
    })

    wyrd.style("launcher_list", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 8, vertical = 8 },
    })

    wyrd.style("launcher_empty", {
        foreground = colors.text_muted,
        font_size  = 12,
    })

    -- ============================================================
    -- 7. CALENDAR & CLOCK MODULE STYLES
    -- ============================================================

    wyrd.style("cal_banner", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 8 },
    })

    wyrd.style("cal_title", {
        foreground = colors.text_primary,
        font_size  = 13,
        font_weight = "bold",
    })

    wyrd.style("cal_grid", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 8, vertical = 8 },
    })

    wyrd.style("cal_day", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 6,
        padding    = { horizontal = 6, vertical = 4 },
        hover      = { background = colors.surface_2, foreground = colors.text_primary },
    })

    wyrd.style("cal_day_today", {
        background = colors.primary,
        foreground = colors.bg_deep,
        font_weight = "bold",
        radius     = 6,
        padding    = { horizontal = 6, vertical = 4 },
    })

    wyrd.style("cal_day_weekend", {
        background = colors.glass,
        foreground = colors.highlight,
        radius     = 6,
        padding    = { horizontal = 6, vertical = 4 },
    })

    wyrd.style("cal_day_dim", {
        foreground = colors.text_muted,
        radius     = 6,
    })

    wyrd.style("cal_nav", {
        background = colors.surface_2,
        foreground = colors.text_secondary,
        radius     = 6,
        padding    = { horizontal = 8, vertical = 4 },
        hover      = { background = colors.surface_3, foreground = colors.text_primary },
    })

    -- ============================================================
    -- 8. SYSTEM & DIAGNOSTICS MODULE STYLES
    -- ============================================================

    wyrd.style("sys_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
    })

    wyrd.style("sys_title", {
        foreground = colors.primary,
        font_size  = 12,
        font_weight = "bold",
    })

    wyrd.style("sys_val", {
        foreground = colors.text_primary,
        font_size  = 14,
        font_weight = "bold",
    })

    wyrd.style("sys_bar", {
        background = colors.surface_3,
        foreground = colors.primary,
        radius     = 9999,
    })

    wyrd.style("sys_ring", {
        foreground = colors.primary,
        background = colors.surface_3,
    })

    wyrd.style("ring_primary", {
        foreground = colors.primary,
        background = colors.surface_3,
    })

    wyrd.style("ring_secondary", {
        foreground = colors.secondary,
        background = colors.surface_3,
    })

    -- ============================================================
    -- 9. POWER MENU MODULE STYLES
    -- ============================================================

    wyrd.style("power_card", {
        background = colors.surface_1,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 10 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.border_glow,
        },
    })

    wyrd.style("power_danger", {
        background = "rgba(243, 139, 168, 0.14)",
        foreground = colors.error,
        outline    = "1px solid " .. colors.error,
        radius     = 12,
        padding    = { horizontal = 12, vertical = 8 },
        font_weight = "bold",
        hover      = {
            background = "rgba(243, 139, 168, 0.24)",
            shadow     = { radius = 16, opacity = 0.35, color = colors.error },
        },
    })

    wyrd.style("power_title", {
        foreground = colors.text_primary,
        font_size  = 13,
        font_weight = "bold",
    })

    wyrd.style("power_btn", {
        background = colors.surface_2,
        foreground = colors.text_primary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        hover      = { background = colors.surface_3, outline = "1px solid " .. colors.border_glow },
    })

    -- ============================================================
    -- 10. SYSTEM TRAY CONTEXT MENU STYLES
    -- ============================================================

    wyrd.style("tray_menu_popup", {
        background = colors.bg_panel,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_subtle,
        shadow     = shadows.popup,
        radius     = 16,
        padding    = { horizontal = 8, vertical = 8 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("menu_header", {
        foreground = colors.primary,
        font_size  = 11,
        font_weight = "bold",
    })

    wyrd.style("menu_section_header", {
        foreground = colors.secondary,
        font_size  = 10,
        font_weight = "bold",
        padding    = { horizontal = 6, vertical = 4 },
    })

    wyrd.style("menu_item", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_primary,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.primary,
            outline    = "1px solid " .. colors.border_glow,
        },
    })

    wyrd.style("menu_item_disabled", {
        background = "rgba(0, 0, 0, 0)",
        foreground = colors.text_muted,
        radius     = 8,
        padding    = { horizontal = 10, vertical = 6 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("menu_separator", {
        background = colors.border_subtle,
        radius     = 1,
    })

    -- ============================================================
    -- 15. QUICK SETTINGS (CONTROL CENTER)
    -- ============================================================

    wyrd.style("quicksettings_popup", {
        background = gradients.bar,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_subtle,
        shadow     = shadows.popup,
        radius     = 24,
        padding    = { horizontal = 16, vertical = 16 },
        font       = font,
        font_size  = 11,
    })

    wyrd.style("header_title", {
        foreground = colors.text_primary,
        font       = font,
        font_size  = 13,
    })

    wyrd.style("quick_toggle", {
        background = colors.surface_1,
        foreground = colors.text_secondary,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 8 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
            outline    = "1px solid " .. colors.border_glow,
        },
        active     = {
            background = colors.surface_3,
            foreground = colors.primary,
        },
    })

    wyrd.style("quick_toggle_active", {
        background = colors.surface_3,
        foreground = colors.primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 14,
        padding    = { horizontal = 12, vertical = 8 },
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
        },
    })

    wyrd.style("qs_slider_card", {
        background = colors.surface_1,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_subtle,
        radius     = 14,
        font       = font,
        font_size  = 11,
    })

    wyrd.style("icon_label", {
        foreground = colors.primary,
        font       = font,
        font_size  = 13,
    })

    wyrd.style("value_label", {
        foreground = colors.text_secondary,
        font       = font,
        font_size  = 10,
    })

    wyrd.style("qs_media_card", {
        background = gradients.surface,
        foreground = colors.text_primary,
        outline    = "1px solid " .. colors.border_glow,
        radius     = 16,
        font       = font,
        font_size  = 11,
    })

    wyrd.style("media_title", {
        foreground = colors.text_primary,
        font       = font,
        font_size  = 11,
    })

    wyrd.style("media_artist", {
        foreground = colors.text_muted,
        font       = font,
        font_size  = 10,
    })

    wyrd.style("media_control_btn", {
        background = colors.glass,
        foreground = colors.text_secondary,
        radius     = 9999,
        font       = font,
        font_size  = 11,
        hover      = {
            background = colors.surface_2,
            foreground = colors.text_primary,
        },
    })

    wyrd.style("media_control_btn_primary", {
        background = colors.primary,
        foreground = colors.text_inverse,
        radius     = 9999,
        font       = font,
        font_size  = 12,
        hover      = {
            background = colors.secondary,
        },
    })
end

return M
