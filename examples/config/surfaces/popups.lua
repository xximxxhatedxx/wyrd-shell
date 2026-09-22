-- =============================================================================
-- AETHERIA: POPUP SURFACES
-- =============================================================================
-- Dynamic Module-Driven Popups (Zero static placeholders)
-- Popups are populated with real-time data from native modules & D-Bus.
-- =============================================================================

-- ------------------------------------------------------------
-- 1. LAUNCHER ("Command Palette")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "launcher",
    anchor_to = "launcher_btn",
    layer = "top",
    anchor = { "top", "left" },
    margin = { top = 58, left = 16 },
    width = 480,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 1b. CLIPBOARD ("Clipboard History")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "clipboard",
    module_channel = "clipboard",
    layer = "top",
    anchor = { "top" },
    margin = { top = 58 },
    width = 460,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 2. AUDIO ("Acoustic Console")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "audio",
    module_channel = "audio",
    anchor_to = "audio",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 2b. AUDIO DEVICES ("Output Selector")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "audio-devices",
    module_channel = "audio",
    anchor_to = "audio",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 3. NETWORK ("Network Radar")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "network",
    module_channel = "network",
    anchor_to = "network",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 3b. NETWORK INFO ("Network Details")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "network-info",
    module_channel = "network",
    anchor_to = "network",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 4. BLUETOOTH ("Pairing Console")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "bluetooth",
    module = "bluetooth",
    module_channel = "bluetooth",
    anchor_to = "bluetooth",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 340,
    height = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 5. BATTERY ("Energy Matrix")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "battery",
    module_channel = "battery",
    anchor_to = "battery",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 320,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 6. CALENDAR ("Chronometric Portal")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "calendar",
    module = "clock",
    module_channel = "clock",
    anchor_to = "clock",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 7. SYSTEM ("System Diagnostics")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "system",
    module_channel = "system",
    anchor_to = "system",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 8. NOTIFICATIONS ("Notification Center")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "notifications",
    module_channel = "notifications",
    anchor_to = "notifications",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 9. POWER ("Control Matrix")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "power",
    module = "power-menu",
    module_channel = "power-menu",
    anchor_to = "power_btn",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 320,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 10. MICROPHONE ("Audio Input Console")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "microphone",
    module = "audio",
    module_channel = "audio",
    anchor_to = "microphone",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 11. TOAST ("Notification Banner")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "toast",
    module = "notifications",
    module_channel = "notifications",
    anchor_to = "notifications",
    layer = "overlay",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 12. WI-FI AUTH ("Network Authentication Modal")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "wifi-auth",
    module = "network",
    module_channel = "network",
    anchor_to = "network",
    layer = "overlay",
    keyboard = "on_demand",
    anchor = {},
    margin = { top = 0, right = 0, bottom = 0, left = 0 },
    width = 380,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 13. TRAY MENU ("System Tray Context Menu")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "tray_menu",
    module = "tray",
    module_channel = "tray",
    anchor_to = "tray",
    layer = "top",
    anchor = { "top", "left" },
    margin = { top = 58, left = 200 },
    width = 250,
    style = "tray_menu_popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 14. MPRIS ("Media Player Console")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "mpris",
    module = "mpris",
    module_channel = "mpris",
    anchor_to = "mpris",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 360,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 15. BRIGHTNESS ("Display Backlight Console")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "brightness",
    module = "brightness",
    module_channel = "brightness",
    anchor_to = "brightness",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 340,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 16. KEYBOARD ("Keyboard Layout Switcher")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "keyboard",
    module = "keyboard",
    module_channel = "keyboard",
    anchor_to = "keyboard",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 300,
    style = "popup",
    widgets = {},
})

-- ------------------------------------------------------------
-- 17. QUICK SETTINGS ("Unified Control Center")
-- ------------------------------------------------------------
wyrd.create({
    type = "popup",
    name = "quicksettings",
    anchor_to = "quicksettings_btn",
    layer = "top",
    anchor = { "top", "right" },
    margin = { top = 58, right = 16 },
    width = 380,
    style = "quicksettings_popup",
    widgets = {
        {
            type = "container",
            layout = { mode = "flex_col", gap = 12, padding = { top = 16, right = 16, bottom = 16, left = 16 } },
            children = {
                -- 1. Header: Status / Battery / Power
                {
                    type = "container",
                    layout = { mode = "flex_row", align = "center", justify = "space_between" },
                    children = {
                        {
                            type = "container",
                            layout = { mode = "flex_row", align = "center", gap = 8 },
                            children = {
                                {
                                    type = "text",
                                    text = "󰣇 Quick Settings",
                                    style = "header_title",
                                },
                            },
                        },
                        {
                            type = "container",
                            layout = { mode = "flex_row", align = "center", gap = 8 },
                            children = {
                                {
                                    type = "button",
                                    id = "qs_battery",
                                    text = "󰁹 {battery.percent}%",
                                    action = "popup:battery",
                                    style = "chip",
                                },
                                {
                                    type = "button",
                                    id = "qs_power",
                                    text = "⏻",
                                    action = "popup:power",
                                    style = "power_round",
                                    layout = { width = 30, height = 30, justify = "center", align = "center" },
                                },
                            },
                        },
                    },
                },

                -- 2. Quick Toggles (2x2 Grid)
                {
                    type = "container",
                    layout = { mode = "flex_col", gap = 8 },
                    children = {
                        {
                            type = "container",
                            layout = { mode = "flex_row", gap = 8 },
                            children = {
                                -- Wi-Fi Toggle
                                {
                                    type = "button",
                                    id = "qs_wifi",
                                    text = "󰤨  {network.ssid}",
                                    action = "popup:toggle network",
                                    style = "quick_toggle",
                                    layout = { weight = 1, height = 44, align = "center", justify = "start" },
                                },
                                -- Bluetooth Toggle
                                {
                                    type = "button",
                                    id = "qs_bluetooth",
                                    text = "󰂯  Bluetooth",
                                    action = "popup:toggle bluetooth",
                                    style = "quick_toggle",
                                    layout = { weight = 1, height = 44, align = "center", justify = "start" },
                                },
                            },
                        },
                        {
                            type = "container",
                            layout = { mode = "flex_row", gap = 8 },
                            children = {
                                -- Do Not Disturb Toggle
                                {
                                    type = "button",
                                    id = "qs_dnd",
                                    text = "󰂚  Do Not Disturb",
                                    action = "event:notifications:dnd_toggle:click",
                                    style = "quick_toggle",
                                    layout = { weight = 1, height = 44, align = "center", justify = "start" },
                                },
                            },
                        },
                    },
                },

                -- 3. Sliders Section: Volume and Brightness
                {
                    type = "container",
                    layout = { mode = "flex_col", gap = 10 },
                    children = {
                        -- Volume Slider Row
                        {
                            type = "container",
                            style = "qs_slider_card",
                            layout = { mode = "flex_row", align = "center", gap = 10, padding = { top = 8, right = 12, bottom = 8, left = 12 } },
                            children = {
                                {
                                    type = "text",
                                    text = "󰕾",
                                    style = "icon_label",
                                },
                                {
                                    type = "slider",
                                    id = "qs_volume_slider",
                                    bind = "audio.volume",
                                    value = 50,
                                    min = 0,
                                    max = 100,
                                    on_change = "event:audio:volume_slider:{value}",
                                    action = "event:audio:volume_slider:{value}",
                                    layout = { weight = 1, height = 18 },
                                },
                                {
                                    type = "text",
                                    text = "{audio.volume}%",
                                    style = "value_label",
                                },
                            },
                        },
                        -- Brightness Slider Row
                        {
                            type = "container",
                            style = "qs_slider_card",
                            layout = { mode = "flex_row", align = "center", gap = 10, padding = { top = 8, right = 12, bottom = 8, left = 12 } },
                            children = {
                                {
                                    type = "text",
                                    text = "󰃠",
                                    style = "icon_label",
                                },
                                {
                                    type = "slider",
                                    id = "qs_brightness_slider",
                                    bind = "brightness.percent",
                                    value = 75,
                                    min = 5,
                                    max = 100,
                                    on_change = "event:brightness:slider_brightness:{value}",
                                    action = "event:brightness:slider_brightness:{value}",
                                    layout = { weight = 1, height = 18 },
                                },
                                {
                                    type = "text",
                                    text = "{brightness.percent}%",
                                    style = "value_label",
                                },
                            },
                        },
                    },
                },

                -- 4. Media Player Mini-Card (MPRIS)
                {
                    type = "container",
                    style = "qs_media_card",
                    layout = { mode = "flex_col", gap = 8, padding = { top = 10, right = 12, bottom = 10, left = 12 } },
                    children = {
                        {
                            type = "container",
                            layout = { mode = "flex_row", align = "center", justify = "space_between" },
                            children = {
                                {
                                    type = "container",
                                    layout = { mode = "flex_col", gap = 2, weight = 1, min_height = 36 },
                                    children = {
                                        {
                                            type = "text",
                                            text = "{mpris.title ? mpris.title : 'No media playing'}",
                                            style = "media_title",
                                        },
                                        {
                                            type = "text",
                                            text = "{mpris.artist ? mpris.artist : 'Media Player'}",
                                            style = "media_artist",
                                        },
                                    },
                                },
                                {
                                    type = "container",
                                    layout = { mode = "flex_row", align = "center", gap = 6 },
                                    children = {
                                        {
                                            type = "button",
                                            id = "qs_mpris_prev",
                                            text = "󰒮",
                                            action = "event:mpris:mpris_prev:click",
                                            style = "media_control_btn",
                                            layout = { width = 28, height = 28, justify = "center", align = "center" },
                                        },
                                        {
                                            type = "button",
                                            id = "qs_mpris_play",
                                            text = "{mpris.playing ? '󰏤' : '󰐊'}",
                                            action = "event:mpris:mpris_play_pause:click",
                                            style = "media_control_btn_primary",
                                            layout = { width = 32, height = 32, justify = "center", align = "center" },
                                        },
                                        {
                                            type = "button",
                                            id = "qs_mpris_next",
                                            text = "󰒭",
                                            action = "event:mpris:mpris_next:click",
                                            style = "media_control_btn",
                                            layout = { width = 28, height = 28, justify = "center", align = "center" },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        },
    },
})



