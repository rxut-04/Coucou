#!/bin/bash
# Terminal finish sensor - bash/zsh hook
# Records command start time and notifies Coucou when a command finishes

# Report a command that ran for at least $1 seconds to the Coucou relay.
__coucou_report() {
    local exit_code="$1" duration="$2" cmd="$3"
    # Commands that are the terminal itself, not a task that finishes.
    case "$cmd" in
        vim|vi|nvim|nano|emacs|less|more|man|top|htop|btop|tmux|screen|ssh|mosh) return ;;
    esac
    [ "$duration" -lt "${COUCOU_TERMINAL_MIN_DURATION:-2}" ] && return
    # The command name only — never the arguments, which hold paths and prompts.
    local name="${cmd%% *}"
    local agent="terminal"
    if [ -n "$KONSOLE_VERSION" ] || [ -n "$KONSOLE_DBUS_SERVICE" ]; then
        agent="konsole"
    fi
    printf '{"hook_event_name":"SessionEnd","session_id":"%s","coucou_agent":"%s","cmd":"%s","message":"Command finished: %s took %ss (exit %s)","duration":%s,"exit_code":%s}\n' \
        "$agent" "$agent" "$name" "$name" "$duration" "$exit_code" "$duration" "$exit_code" \
        | ~/.local/share/coucou/bin/coucou-hook --agent "$agent" 2>/dev/null
}

if [ -n "$ZSH_VERSION" ]; then
    # zsh: preexec gets the command line before it runs, precmd after.
    __coucou_start_time=0
    __coucou_last_command=""
    __coucou_preexec() {
        __coucou_last_command="$1"
        __coucou_start_time=$(date +%s)
    }
    __coucou_precmd() {
        local exit_code=$?
        if [ "${__coucou_start_time:-0}" -gt 0 ] && [ -n "$__coucou_last_command" ]; then
            __coucou_report "$exit_code" "$(( $(date +%s) - __coucou_start_time ))" "$__coucou_last_command"
        fi
        __coucou_start_time=0
        __coucou_last_command=""
    }
    autoload -Uz add-zsh-hook
    add-zsh-hook preexec __coucou_preexec
    add-zsh-hook precmd __coucou_precmd
elif [ -n "$BASH_VERSION" ]; then
    # bash: the DEBUG trap fires before every simple command — including the
    # PROMPT_COMMAND itself — so two guards keep the timings honest:
    #   __coucou_armed is raised by precmd, after it read the exit code, and
    #   only a command seen while it is raised starts a timer. That way a line
    #   of several commands is timed from its first, and the prompt's own
    #   bookkeeping never starts a timer of its own.
    __coucou_armed=0
    __coucou_start_time=0
    __coucou_last_command=""

    __coucou_preexec() {
        [ "$__coucou_armed" = 1 ] || return 0
        __coucou_last_command="$BASH_COMMAND"
        __coucou_start_time=$(date +%s)
        __coucou_armed=0
    }
    __coucou_precmd() {
        local exit_code=$?
        if [ "$__coucou_start_time" -gt 0 ] && [ -n "$__coucou_last_command" ]; then
            __coucou_report "$exit_code" "$(( $(date +%s) - __coucou_start_time ))" "$__coucou_last_command"
        fi
        __coucou_start_time=0
        __coucou_last_command=""
        __coucou_armed=1
    }
    trap '__coucou_preexec "$BASH_COMMAND"' DEBUG
    case ";${PROMPT_COMMAND:-};" in
        *__coucou_precmd*) ;; # already installed (re-sourced)
        *) PROMPT_COMMAND="__coucou_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
    esac
fi
