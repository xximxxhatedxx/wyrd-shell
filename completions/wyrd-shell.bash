# Bash completion script for wyrd-shell

_wyrd_shell() {
    local cur prev words cword
    _init_completion || return

    local opts="--config -c --daemonize --debug --log-level --render-mode --theme --layout --action --doctor --completions -h --help -V --version"

    case "${prev}" in
        --config|-c)
            _filedir '@(lua)'
            return
            ;;
        --render-mode)
            COMPREPLY=($(compgen -W "auto gpu cpu" -- "${cur}"))
            return
            ;;
        --log-level)
            COMPREPLY=($(compgen -W "trace debug info warn error" -- "${cur}"))
            return
            ;;
        --theme)
            COMPREPLY=($(compgen -W "catppuccin aetheria nord gruvbox tokyo-night" -- "${cur}"))
            return
            ;;
        --layout)
            COMPREPLY=($(compgen -W "top left" -- "${cur}"))
            return
            ;;
        --completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "${cur}"))
            return
            ;;
        --action)
            local actions="doctor popup:launcher popup:network popup:bluetooth popup:audio popup:microphone popup:battery popup:system popup:calendar popup:mpris popup:quicksettings popup:power popup:toggle\ launcher popup:toggle\ network popup:toggle\ bluetooth popup:toggle\ audio popup:toggle\ microphone popup:toggle\ battery popup:toggle\ system popup:toggle\ calendar popup:toggle\ mpris popup:toggle\ quicksettings popup:toggle\ power popup:close theme:set\ catppuccin theme:set\ aetheria theme:set\ nord theme:set\ gruvbox theme:set\ tokyo-night theme:toggle layout:set\ top layout:set\ left layout:toggle reload"
            COMPREPLY=($(compgen -W "${actions}" -- "${cur}"))
            return
            ;;
    esac

    if [[ "${cur}" == -* ]]; then
        COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
        return
    fi
}

complete -F _wyrd_shell wyrd-shell
