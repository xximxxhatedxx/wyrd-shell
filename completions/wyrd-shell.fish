# Fish completions for wyrd-shell

complete -c wyrd-shell -s c -l config -r -F -d "Path to init.lua configuration"
complete -c wyrd-shell -l daemonize -d "Run as background daemon"
complete -c wyrd-shell -l debug -d "Enable debug overlay"
complete -c wyrd-shell -l log-level -x -a "trace debug info warn error" -d "Log level"
complete -c wyrd-shell -l render-mode -x -a "auto gpu cpu" -d "Render backend"
complete -c wyrd-shell -l theme -x -a "catppuccin aetheria nord gruvbox tokyo-night" -d "Switch active theme"
complete -c wyrd-shell -l layout -x -a "top left" -d "Switch active bar layout"
complete -c wyrd-shell -l doctor -d "Run system and environment diagnostics"
complete -c wyrd-shell -l completions -x -a "bash zsh fish" -d "Output shell completion script"

set -l actions \
    "doctor\tRun system diagnostics" \
    "popup:launcher\tOpen launcher popup" \
    "popup:network\tOpen network selector popup" \
    "popup:bluetooth\tOpen bluetooth manager popup" \
    "popup:audio\tOpen audio output selector" \
    "popup:microphone\tOpen microphone selector" \
    "popup:battery\tOpen battery details popup" \
    "popup:system\tOpen system monitor popup" \
    "popup:calendar\tOpen calendar and date popup" \
    "popup:mpris\tOpen media player console" \
    "popup:quicksettings\tOpen unified control center popup" \
    "popup:power\tOpen power session menu" \
    "popup:close\tClose any open popups" \
    "theme:set catppuccin\tSwitch to Catppuccin Mocha theme" \
    "theme:set aetheria\tSwitch to Aetheria Etheric theme" \
    "theme:set nord\tSwitch to Nord theme" \
    "theme:set gruvbox\tSwitch to Gruvbox Dark theme" \
    "theme:set tokyo-night\tSwitch to Tokyo Night theme" \
    "theme:toggle\tToggle between themes" \
    "layout:set top\tSwitch to top bar layout" \
    "layout:set left\tSwitch to left bar layout" \
    "layout:toggle\tToggle bar layout" \
    "reload\tReload shell configuration"

complete -c wyrd-shell -l action -x -a "$actions" -d "Send action to running wyrd-shell daemon"
complete -c wyrd-shell -s h -l help -d "Print help message"
complete -c wyrd-shell -s V -l version -d "Print version information"
