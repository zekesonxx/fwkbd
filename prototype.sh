#!/usr/bin/env bash
set -e

STEP=4
BRIGHTNESS=50
IDLE_THRESHOLD=2000

set_backlight() {
    sudo ectool pwmsetkblight "$1" >>/dev/null
}

# reset the backlight on close
trap_ctrlc() {
    set_backlight "$BRIGHTNESS"
    exit 0
}

trap trap_ctrlc INT

fade_in() {
    for i in $(seq 0 "$STEP" "$BRIGHTNESS"); do
        set_backlight "$i"
    done
}

fade_out() {
    for i in $(seq 0 "1" "$BRIGHTNESS" | sort -nr); do
        set_backlight "$i"
    done
}

wait_for_idle() {
    while [[ "$(xprintidle)" -lt "$IDLE_THRESHOLD" ]]; do
        sleep 0.5
    done
}

wait_for_not_idle() {
    while [[ "$(xprintidle)" -gt "$IDLE_THRESHOLD" ]]; do
        sleep 0.01
    done
}

while true; do
    wait_for_idle
    fade_out
    wait_for_not_idle
    fade_in
done