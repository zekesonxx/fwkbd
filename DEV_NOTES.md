# Don't monopolize the EC handle
The user may have other things they want talking to the ectool, such as adjusting fan speeds, other LEDs, or anything else `ectool` can do. As such, we shouldn't monopolize our time with the EC handle.

To achieve this, we only open a handle to the embedded controller when we first start changing the backlight, and release it when we're done adjusting it.

# uled load testing
```sh
while true; do brightnessctl -d '*kbd*' s $((1 + RANDOM % 100)); done
```