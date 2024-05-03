# fwkbd: Framework Keyboard Backlight Daemon
A daemon to automatically brighten/dim your Framework keyboard backlight.

Dims your keyboard automatically after a period of time idle, and then undims it when you touch the keyboard or move the pointer.

Inspired heavily by the automatic dimming behavior of Macbook keyboards.

# Features
* Dim when idle, brighten when not idle
* Adjust the (non-idle) brightness on-the-fly using any Linux LED control software, thanks to [uleds] (`/sys/class/leds/fwkbd::kbd_backlight`)
* Adjust fade-in and fade-out timers and brightness curves via CLI options
* Optionally ignore any trackpad/pointer events, and only respond to keyboard events
* Lightweight, uses ~6MB of RAM and almost no CPU

## Dev Notes
Things I'd like to add in the future:
* **Better libinput event handling**: right now libinput events are queued up back to the main thread, there's really no reason for this, they could be filtered on the libinput thread and have very little info sent back over.
* **Filter to specific event sources**: e.x. to only allow input on the Framework's keyboard and trackpad to reset the idle timer, ignoring any external keyboards or mice.
* **Direct EC communication**: This tool uses ectool under the hood to speak to the EC. I'd like to have it talk to the EC directly to reduce overhead.

  [uleds]: https://www.kernel.org/doc/html/latest/leds/uleds.html