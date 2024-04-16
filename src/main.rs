use std::borrow::Borrow;
use std::process::Stdio;
use clap::Parser;
use libinput::LibinputEventListener;
use log::{debug, error, info, trace};
use uleds::Uleds;
use std::time::{Duration, Instant};

use keyframe::{ease_with_scaled_time, EasingFunction};

use anyhow::Result;

mod uleds;
mod libinput;
mod cli;

/// Execute `ectool pwmsetkblight <level>`
async fn ectool_pwmsetkblight(level: u8) -> Result<()> {
    let cmd = tokio::process::Command::new("ectool")
        // we specify the interface because otherwise ectool has to figure it out itself
        // and it takes a while: ~12ms vs ~6ms
        .args(["--interface=lpc", "--name=cros_ec"])
        .arg("pwmsetkblight")
        .arg(level.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .output().await?;
    if cmd.status.success() {
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
    fade_in: Duration,
    /// Time to fade out the keyboard, in seconds
    fade_out: Duration,
    uleds: bool,
    ease_in: cli::KeyframeFunction,
    ease_out: cli::KeyframeFunction,
}

impl Fwkbd {
    pub fn new(args: &cli::Args) -> Self {
        Fwkbd {
            _libinput: LibinputEventListener::new(),
            state: State::NotIdle,
            current_backlight: args.brightness,
            backlight: args.brightness,
            timeout: Duration::from_secs_f32(args.timeout),
            fade_in: Duration::from_secs_f32(args.fade_in),
            fade_out: Duration::from_secs_f32(args.fade_out),
            uleds: !args.no_uleds,
            ease_in: args.ease_in,
            ease_out: args.ease_out,
        }
    }

    /// returns `true` if uleds is present and could wait
    /// returns `false` immediately if uleds is None
    async fn wait_for_uleds(uleds: &Option<Uleds>) -> bool {
        if let Some(ref uleds) = uleds {
            uleds.wait_for_update().await;
            true
        } else {
            false
        }
    }

    pub async fn set_backlight(&mut self, level: u8) -> Result<()> {
        trace!("set_backlight({level})");
        ectool_pwmsetkblight(level).await?;
        self.current_backlight = level;
        Ok(())
    }

    pub async fn async_loop(&mut self) -> Result<()> {
        use State::*;

        let uleds = if self.uleds {
            Uleds::new(self.backlight).await.ok()
        } else {
            None
        };

        // reset to max backlight
        self.set_backlight(self.backlight).await?;

        let timeout = self.timeout;
    
        loop {
            if let Some(ref uleds) = uleds {
                // get the uled brightness once to prevent race conditions
                let uleds_brightness = uleds.brightness();
                if self.backlight != uleds_brightness {
                    // user changed the led brightness
                    self.state = NotIdle;
                    self.backlight = uleds_brightness;
                    self.fade_accordingly().await?;
                }
            }

            match self.state {
                Idle => {
                    tokio::select! {
                        _ = self.get_next_event() => {
                            info!("newly not idle");
                            self.fade_accordingly().await?;

                        }
                        _ = Self::wait_for_uleds(&uleds) => {
                            //brightness update
                        }
                    }
                },
                NotIdle => {
                    tokio::select! {
                        _ = self.get_next_event() => {
                            //idle timer reset
                        }
                        _ = Self::wait_for_uleds(&uleds) => {
                            //brightness update
                        }
                        _ = tokio::time::sleep(timeout) => {
                            self.state = Idle;
                            info!("got sleep");
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
        trace!("fade_accordingly()");
        match self.state {
            State::Idle => if self.current_backlight != 0 {
                tokio::task::unconstrained(self.fade_backlight(0, self.fade_out, self.ease_out)).await?;
            },
            State::NotIdle => if self.current_backlight != self.backlight {
                tokio::task::unconstrained(self.fade_backlight(self.backlight, self.fade_in, self.ease_in)).await?;
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
    pub async fn fade_backlight<F: EasingFunction>(&mut self, goal: u8, time: Duration, func: impl Borrow<F> + Copy) -> Result<()> {
        trace!("fade_backlight(goal={goal})");
        let time_start = Instant::now();
        let starting_backlight = self.current_backlight as f32;
        let goal_backlight = goal as f32;

        let mut elapsed;
        // instant when we started the animation
        let mut iteration_start;

        // if we're fading down (to idle)
        // we want to be able to interrupt it and immediately fade back
        let interruptable = starting_backlight > goal_backlight;

        #[cfg(debug_assertions)]
        let mut ectool_avg = vec![];

        loop {
            iteration_start = Instant::now();

            elapsed = time_start.elapsed();

            if elapsed >= time {
                if self.current_backlight != goal {
                    self.set_backlight(goal).await?;
                }
                break;
            }

            let tween = ease_with_scaled_time(func, starting_backlight, goal_backlight, elapsed.as_secs_f32(), time.as_secs_f32()) as u8;
            if tween == self.current_backlight {
                debug!("tweened too fast");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            #[cfg(debug_assertions)]
            let i = Instant::now();
            self.set_backlight(tween).await?;
            #[cfg(debug_assertions)]
            ectool_avg.push(i.elapsed());
            debug!("tween={tween}, elapsed={elapsed:?}");

            if tween == goal {
                break;
            }

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
        #[cfg(debug_assertions)]
        debug!("ectool averaged {:?}", ectool_avg.iter().sum::<Duration>().checked_div(ectool_avg.len() as u32).unwrap());
        Ok(())
    }
}

#[tokio::main]
//#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    env_logger::init();

    // start the program
    let mut fwkbd = Fwkbd::new(&args);

    tokio::select! {
        e = fwkbd.async_loop() => {
            e?;
        }
        _ = tokio::signal::ctrl_c() => {
            error!("got SIGTERM, resetting backlight and closing");
            fwkbd.set_backlight(fwkbd.backlight).await?;
            std::process::exit(0);
        }
    }
    //fwkbd.async_loop(true).await?;

    Ok(())
}
