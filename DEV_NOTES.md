# uled load testing
```sh
while true; do brightnessctl -d '*kbd*' s $((1 + RANDOM % 100)); done
```