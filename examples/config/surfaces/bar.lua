-- =============================================================================
-- AETHERIA: MAIN BAR SURFACE
-- =============================================================================
-- Floating Capsule Bar
-- =============================================================================

wyrd.create({
    type = "bar",
    name = "main_bar",
    layer = "top",
    height = 38,
    anchor = { "top", "left", "right" },
    margin = { top = 10, left = 20, right = 20 },
    exclusive_zone = 48,
    
    style = "bar",
    
    widgets = {
        -- ========== LEFT SECTION (Launcher, Wi-Fi, Bluetooth, Tray) ==========
        {
            type = "container",
            layout = { mode = "flex_row", gap = 8, align = "center", weight = 1, justify = "start" },
            children = {
                -- Launcher Button
                {
                    type = "button",
                    id = "launcher_btn",
                    text = "󰀻",
                    style = "launcher_round",
                    action = "popup:launcher",
                    layout = { width = 32, height = 28, justify = "center", align = "center" },
                },
                {
                    type = "button",
                    id = "mpris",
                    text = "󰎆 Media",
                    tooltip = "Media Player",
                    action = "popup:mpris",
                    on_right_click = "event:mpris:mpris_play_pause:click",
                    style = "chip",
                },
                
                -- Wi-Fi / Network
                {
                    type = "button",
                    id = "network",
                    text = "󰤨 {network.ssid}",
                    action = "popup:network",
                    style = "chip",
                },
                
                -- Bluetooth
                {
                    type = "button",
                    id = "bluetooth",
                    text = "󰂯",
                    action = "popup:bluetooth",
                    style = "chip",
                },
                
                -- System Tray
                {
                    type = "tray",
                    id = "tray",
                    style = "clean_item",
                    layout = { mode = "flex_row", align = "center", gap = 6 },
                },
            }
        },
        
        -- ========== CENTER SECTION (Workspaces) ==========
        {
            type = "workspaces",
            id = "workspaces",
            layout = { mode = "flex_row", gap = 4, align = "center", justify = "center" },
        },
        
        -- ========== RIGHT SECTION (Performance, Speaker, Microphone, Battery, Notifications, Power) ==========
        {
            type = "container",
            layout = { mode = "flex_row", gap = 6, align = "center", weight = 1, justify = "end" },
            children = {
                -- Performance / System Monitor (CPU & RAM)
                -- {
                --     type = "button",
                --     id = "system",
                --     text = "󰍛 {system.cpu}% • 󰚌 {system.mem}%",
                --     action = "popup:system",
                --     style = "chip",
                -- },
                
                -- Speaker (Audio Output)
                {
                    type = "button",
                    id = "audio",
                    text = "󰕾 {audio.volume}%",
                    action = "popup:audio",
                    style = "chip",
                },
                
                -- Microphone (Audio Input)
                {
                    type = "button",
                    id = "microphone",
                    text = "󰍬 {microphone.volume}%",
                    action = "popup:microphone",
                    style = "chip",
                },
                
                -- Battery
                {
                    type = "button",
                    id = "battery",
                    text = "󰁹 {battery.percent}%",
                    action = "popup:battery",
                    style = "chip",
                },
                
                -- Display Brightness (Also adjustable via Quick Settings)
                -- {
                --     type = "button",
                --     id = "brightness",
                --     text = "󰃟 {brightness.percent}%",
                --     action = "popup:brightness",
                --     style = "chip",
                -- },
                
                -- Keyboard Layout
                {
                    type = "button",
                    id = "keyboard",
                    text = "󰌌 {keyboard.short_name}",
                    action = "popup:keyboard",
                    style = "chip",
                },
                
                -- Notifications
                {
                    type = "button",
                    id = "notifications",
                    text = "󰂚 {notifications.count}",
                    action = "popup:notifications",
                    style = "chip",
                },
                
                -- Clock & Date
                {
                    type = "button",
                    id = "clock",
                    text = "󰥔 {clock.time} • {clock.date}",
                    action = "popup:calendar",
                    style = "chip",
                },
                
                -- Quick Settings / Control Center (Includes Power, Wi-Fi, BT, Sliders)
                {
                    type = "button",
                    id = "quicksettings_btn",
                    text = "󰒓",
                    action = "popup:quicksettings",
                    tooltip = "Quick Settings",
                    style = "chip",
                },
                
                -- Standalone Power Button (Available directly inside Quick Settings)
                -- {
                --     type = "button",
                --     id = "power_btn",
                --     text = "⏻",
                --     action = "popup:power",
                --     style = "power_round",
                --     layout = { width = 32, height = 32, justify = "center", align = "center" },
                -- }
            }
        }
    }
})
