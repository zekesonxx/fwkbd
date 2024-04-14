use std::borrow::Borrow;
use std::process::Stdio;
use libinput::LibinputEventListener;
use uleds::Uleds;
use std::time::{Duration, Instant};

use keyframe::{ease_with_scaled_time, EasingFunction};

use keyframe::functions;

use anyhow::Result;

mod uleds;
mod libinput;

fn ectool_pwmsetkblight_blocking(level: u8) -> Result<()> {
    let i = Instant::now();
    let cmd = std::process::Command::new("ectool")
        .args(["--interface=lpc", "--name=cros_ec"])
        .arg("pwmsetkblight")
        .arg(level.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .output()?;
    if cmd.status.success() {
        println!("ectool took {:?}", i.elapsed());
        Ok(())
    } else {
        anyhow::bail!("ectool error: {}", String::from_utf8_lossy(&cmd.stderr));
    }
}

async fn ectool_pwmsetkblight(level: u8) -> Result<()> {
    let i = Instant::now();
    let cmd = tokio::process::Command::new("ectool")
        .args(["--interface=lpc", "--name=cros_ec"])
        .arg("pwmsetkblight")
        .arg(level.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .output().await?;
    if cmd.status.success() {
        println!("ectool took {:?}", i.elapsed());
        Ok(())
    } else {
        anyhow::bail!("ectool error: {}", String::from_utf8_lossy(&cmd.stderr));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Idle,
    NotIdle
}

struct Fwkbd {
    _libinput: LibinputEventListener,
    state: State,
    current_backlight: u8,
    backlight: u8,
    timeout: Duration,
    /// Time to fade in the keyboard, in seconds
    fade_in: f32,
    /// Time to fade out the keyboard, in seconds
    fade_out: f32,
}

impl Fwkbd {
    pub fn new(timeout: Duration, backlight: u8) -> Self {
        Fwkbd {
            _libinput: LibinputEventListener::new(),
            state: State::NotIdle,
            current_backlight: backlight,
            backlight, timeout,
            fade_in: 0.5,
            fade_out: 1.0
        }
    }

    pub async fn set_backlight(&mut self, level: u8) -> Result<()> {
        ectool_pwmsetkblight(level).await?;
        self.current_backlight = level;
        println!("bl: {level}");
        Ok(())
    }

    pub async fn async_loop(&mut self, uleds: Uleds) -> Result<()> {
        use State::*;

        // reset to max backlight
        self.set_backlight(self.backlight).await?;

        let timeout = self.timeout;
    
        loop {
            // get the uled brightness once to prevent race conditions
            let uleds_brightness = uleds.brightness();
            if self.backlight != uleds_brightness {
                // user changed the led brightness
                self.state = NotIdle;
                self.backlight = uleds_brightness;
                self.fade_accordingly().await?;
            }

            match self.state {
                Idle => {
                    tokio::select! {
                        _ = self.get_next_event() => {
                            println!("newly not idle");
                            self.fade_accordingly().await?;

                        }
                        _ = uleds.wait_for_update() => {
                            //brightness update
                        }
                    }
                },
                NotIdle => {
                    tokio::select! {
                        _ = self.get_next_event() => {
                            //idle timer reset
                        }
                        _ = uleds.wait_for_update() => {
                            //brightness update
                        }
                        _ = tokio::time::sleep(timeout) => {
                            self.state = Idle;
                            println!("got sleep");
                            // this is called twice in case it's interrupted by the user becoming not idle
                            self.fade_accordingly().await?;
                            self.fade_accordingly().await?;
                        }
                    }
                },
            }
        }
    }

    /// If the current backlight doesn't match the correct backlight for the current state, fade accordingly
    /// 
    /// This uses tokio::task::unconstrained so that the animation is never interrupted by other tasks
    /// (to ensure a smooth animation)
    pub async fn fade_accordingly(&mut self) -> Result<()> {
        match self.state {
            State::Idle => if self.current_backlight != 0 {
                tokio::task::unconstrained(self.fade_backlight(0, self.fade_out, functions::EaseOut)).await?;
            },
            State::NotIdle => if self.current_backlight != self.backlight {
                tokio::task::unconstrained(self.fade_backlight(self.backlight, self.fade_in, functions::EaseInQuad)).await?;
            },
        }
        Ok(())
    }

    /// Wait until the next libinput event comes through.
    /// 
    /// Returns `true` if the event made us not idle.
    async fn get_next_event(&mut self) -> Result<bool> {
        use State::*;
        use libinput::LibinputSyncEventType::*;
        let event = self._libinput.next().await?;
        if matches!(event.event_type, DeviceAdded | DeviceRemoved) {
            return Ok(false);
        }
        let changed = self.state == Idle;
        self.state = NotIdle;
        Ok(changed)
    }

    /// Get the next libinput event if there's one available.
    /// 
    /// Doesn't wait for an event, so if there's no events queued, returns almost immediately.
    pub async fn try_update(&mut self) -> Result<bool> {
        use State::*;
        use libinput::LibinputSyncEventType::*;
        while !self._libinput.is_empty() {
            let event = self._libinput.next().await?;
            if matches!(event.event_type, DeviceAdded | DeviceRemoved) {
                continue;
            }
            let changed = self.state == Idle;
            self.state = NotIdle;
            return Ok(changed);
        }
        Ok(false)
    }


    /// Fade the keyboard backlight from the current brightness to the goal brightness
    pub async fn fade_backlight<F: EasingFunction>(&mut self, goal: u8, time_secs: f32, func: impl Borrow<F> + Copy) -> Result<()> {
        let time_start = Instant::now();
        let starting_backlight = self.current_backlight as f32;
        let goal_backlight = goal as f32;

        let mut elapsed;
        // instant when we started the animation
        let mut iteration_start;

        // if we're fading down (to idle)
        // we want to be able to interrupt it and immediately fade back
        let interruptable = starting_backlight > goal_backlight;
        
        loop {
            iteration_start = Instant::now();

            elapsed = time_start.elapsed().as_secs_f32();

            if elapsed >= time_secs {
                if self.current_backlight != goal {
                    self.set_backlight(goal).await?;
                }
                break;
            }

            let tween = ease_with_scaled_time(func, starting_backlight, goal_backlight, elapsed, time_secs);
            let tween = tween as u8;
            if tween == self.current_backlight {
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }
            self.set_backlight(tween).await?;
            println!("tween={tween}, elapsed={elapsed}");
            
            // if we're dimming down, we should be checking for being no longer idle
            if interruptable {
                // try and update
                if self.try_update().await? {
                    // user isn't idle anymore, break out of this fade
                    break;
                }
            }

            // sleep so we don't make a million ectool calls
            let Some(sleep_timer) = Duration::from_millis(50).checked_sub(iteration_start.elapsed()) else { continue; };
            tokio::time::sleep(sleep_timer).await;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let backlight = 100;

    let uleds = Uleds::new(100).await?;

    // println!("{:?}", uleds.poll().await);
    // println!("{:?}", uleds.poll().await);
    // println!("{:?}", uleds.poll().await);
    // println!("{:?}", uleds.poll().await);

    // start the program
    let mut fwkbd = Fwkbd::new(Duration::from_millis(4_000), backlight);
    fwkbd.async_loop(uleds).await?;

    Ok(())
}
