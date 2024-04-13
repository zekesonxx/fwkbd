use std::process::{exit, Stdio};
use std::thread::{current, sleep};
use std::time::{Duration, Instant};

use keyframe::ease_with_scaled_time;
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::screensaver;

use keyframe::functions;

use anyhow::*;
use x11rb::rust_connection::RustConnection;

fn ectool_pwmsetkblight(level: u8) -> Result<()> {
    let cmd = std::process::Command::new("ectool")
        .arg("pwmsetkblight")
        .arg(level.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .output()?;
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
    _x11_conn: RustConnection,
    _x11_screen: u32,
    state: State,
    idle_timer: u32,
    idle_threshold: u32,
    current_backlight: u8,
    backlight: u8,   
}

impl Fwkbd {
    pub fn new(conn: RustConnection, screen: u32, idle_threshold: u32, backlight: u8) -> Self {
        Fwkbd {
            _x11_conn: conn,
            _x11_screen: screen,
            state: State::NotIdle,
            idle_timer: 69420,
            idle_threshold,
            current_backlight: backlight,
            backlight,
        }
    }

    pub fn update_idle_state(&mut self) -> Result<bool> {
        let new_idle = screensaver::query_info(&self._x11_conn, self._x11_screen)?.reply()?.ms_since_user_input;

        //println!("update_idle_state idle_timer={it} new_idle={new_idle} state={is:?}", it=self.idle_timer, is=self.state);
        if new_idle == self.idle_timer {
            return Ok(false);
        }
        let new_state = if new_idle < self.idle_timer {
            // timer went down, but how high was the timer previously?
            if self.idle_timer <= 500 {
                // recent movement, real
                State::NotIdle
            } else {
                // sudden movement after idle
                // need another one soon to confirm
                println!("possible fake movement");
                State::Idle
            }
        } else {
            if new_idle >= self.idle_threshold {
                State::Idle
            } else {
                self.state
            }
        };
        self.idle_timer = new_idle;
        let changed = self.state != new_state;
        self.state = new_state;
        Ok(changed)
    }

    pub fn poll_interval(&self) -> Duration {
        match self.state {
            State::Idle => {
                if self.idle_timer < 500 {
                    // state=idle and low timer means
                    // potential fake event, need to
                    // quickly poll
                    Duration::from_millis(self.idle_timer as u64/2)
                } else {
                    // normal duration for when idle for a quick pick up
                    // since we're not event driven (yet)
                    Duration::from_millis(200)
                }
            },
            State::NotIdle => Duration::from_millis(500),
        }
    }

    pub fn set_backlight(&mut self, level: u8) -> Result<()> {
        ectool_pwmsetkblight(level)?;
        self.current_backlight = level;
        Ok(())
    }

    pub fn main_loop(&mut self) -> Result<()> {
        use State::*;
        let fade_out = 1f32;
        let fade_in = 0.5f32;

        loop {
            // get current idle timer
            let changed = self.update_idle_state()?;
            //println!("{:?}", idle);
    
            if changed && self.state == Idle {
                println!("now idle");
                let start = Instant::now();
                let starting_backlight = self.current_backlight as f32;
                let mut remaining;
                let mut iteration_start;
                loop {
                    iteration_start = Instant::now();
                    remaining = fade_out - start.elapsed().as_secs_f32();
                    if remaining <= 0.0 {
                        if self.current_backlight != 0 {
                            self.set_backlight(0)?;
                        }
                        break;
                    }
                    let tween = ease_with_scaled_time(functions::EaseOut, 0.0, starting_backlight, remaining, fade_out);
                    let tween = tween as u8;
                    if tween == self.current_backlight {
                        sleep(Duration::from_millis(10));
                        continue;
                    }
                    self.set_backlight(tween)?;
                    println!("tween={tween}, remaining={remaining}");
                    // check if they're not idle anymore
                    if self.update_idle_state()? {
                        println!("user no longer idle");
                        break;
                    }
    
                    // sleep so we don't make a million ectool calls
                    let Some(sleep_timer) = Duration::from_millis(50).checked_sub(iteration_start.elapsed()) else { continue; };
                    sleep(sleep_timer);
                }
            }
            
            if changed && self.state == NotIdle {
                println!("now not idle");
    
                let start = Instant::now();
                let starting_backlight = self.current_backlight as f32;
                let goal_backlight = self.backlight as f32;
                let mut remaining;
                let mut iteration_start;
                loop {
                    iteration_start = Instant::now();
                    remaining = fade_in - start.elapsed().as_secs_f32();
                    if remaining <= 0.0 {
                        if self.current_backlight != self.backlight {
                            self.set_backlight(self.backlight)?;
                        }
                        break;
                    }
                    let tween = ease_with_scaled_time(functions::EaseInQuad, goal_backlight, starting_backlight, remaining, fade_in);
                    let tween = tween as u8;
                    if tween == self.current_backlight {
                        sleep(Duration::from_millis(10));
                        continue;
                    }
                    self.set_backlight(tween)?;
                    println!("tween={tween}, remaining={remaining}");
    
                    // sleep so we don't make a million ectool calls
                    let Some(sleep_timer) = Duration::from_millis(50).checked_sub(iteration_start.elapsed()) else { continue; };
                    sleep(sleep_timer);
                }
            }
    
            // sleep so we don't eat all the CPU
            sleep(self.poll_interval());
        }
    }
}

fn main() -> Result<()> {
    let backlight = 60;

    // get an X11 connection
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = conn.setup().roots[screen_num].root;
    if conn.extension_information(screensaver::X11_EXTENSION_NAME)?.is_none() {
        eprintln!("ScreenSaver extension is not supported");
        exit(1);
    }

    // start the program
    let mut fwkbd = Fwkbd::new(conn, screen, 4_000, backlight);
    fwkbd.main_loop()?;

    Ok(())
}
