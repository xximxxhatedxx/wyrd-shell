-- =============================================================================
-- VERTICAL LEFT CAPSULE BAR SURFACE
-- =============================================================================
-- Ergonomic vertical sidebar layout for modern ultra-wide and standard displays
-- =============================================================================

wyrd.create({
    type = "bar",
    name = "main_bar",
    layer = "top",
    width = 46,
    anchor = { "top", "bottom", "left" },
    margin = { top = 12, bottom = 12, left = 12 },
    exclusive_zone = 58,
    
    style = "bar",
    
    widgets = {
        {
            type = "container",
            layout = { mode = "flex_col", justify = "space-between", align = "center", width = "100%", height = "100%" },
            children = {
                -- ========== TOP SECTION (Launcher, Media, Network, Bluetooth, Tray) ==========
                {
                    type = "container",
                    layout = { mode = "flex_col", gap = 8, align = "center", justify = "start" },
            children = {
                -- Launcher Button
                {
                    type = "button",
                    id = "launcher_btn",
                    text = "󰀻",
                    style = "launcher_round",
                    action = "popup:launcher",
                    layout = { width = 36, height = 36, justify = "center", align = "center" },
                },
                -- Media Player Button
                {
                    type = "button",
                    id = "mpris",
                    text = "󰎆",
                    tooltip = "Media Player",
                    action = "popup:mpris",
                    on_right_click = "event:mpris:mpris_play_pause:click",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Wi-Fi / Network
                {
                    type = "button",
                    id = "network",
                    text = "󰤨",
                    tooltip = "{network.ssid}",
                    action = "popup:network",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Bluetooth
                {
                    type = "button",
                    id = "bluetooth",
                    text = "󰂯",
                    tooltip = "Bluetooth",
                    action = "popup:bluetooth",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- System Tray
                {
                    type = "tray",
                    id = "tray",
                    style = "clean_item",
                    layout = { mode = "flex_col", align = "center", gap = 6 },
                },
            }
        },
        
        -- ========== CENTER SECTION (Workspaces) ==========
        {
            type = "workspaces",
            id = "workspaces",
            layout = { mode = "flex_col", gap = 6, align = "center", justify = "center" },
        },
        
        -- ========== BOTTOM SECTION (System, Audio, Mic, Battery, Notifs, Clock, Power) ==========
        {
            type = "container",
            layout = { mode = "flex_col", gap = 6, align = "center", weight = 1, justify = "end" },
            children = {
                -- Performance / System Monitor
                {
                    type = "button",
                    id = "system",
                    text = "󰍛",
                    tooltip = "CPU: {system.cpu}% • RAM: {system.mem}%",
                    action = "popup:system",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Speaker (Audio Output)
                {
                    type = "button",
                    id = "audio",
                    text = "󰕾",
                    tooltip = "Volume: {audio.volume}%",
                    action = "popup:audio",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Microphone (Audio Input)
                {
                    type = "button",
                    id = "microphone",
                    text = "󰍬",
                    tooltip = "Mic: {microphone.volume}%",
                    action = "popup:microphone",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Battery
                {
                    type = "button",
                    id = "battery",
                    text = "󰁹",
                    tooltip = "Battery: {battery.percent}%",
                    action = "popup:battery",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Notifications
                {
                    type = "button",
                    id = "notifications",
                    text = "󰂚",
                    tooltip = "Notifications: {notifications.count}",
                    action = "popup:notifications",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Clock & Date
                {
                    type = "button",
                    id = "clock",
                    text = "󰥔",
                    tooltip = "{clock.time} • {clock.date}",
                    action = "popup:calendar",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Quick Settings / Control Center
                {
                    type = "button",
                    id = "quicksettings_btn",
                    text = "󰒓",
                    tooltip = "Control Center",
                    action = "popup:quicksettings",
                    style = "chip",
                    layout = { width = 34, height = 30, justify = "center", align = "center" },
                },
                -- Power Button
                {
                    type = "button",
                    id = "power_btn",
                    text = "⏻",
                    action = "popup:power",
                    style = "power_round",
                    layout = { width = 34, height = 34, justify = "center", align = "center" },
                }
            }
        }
    }
}
}
})
