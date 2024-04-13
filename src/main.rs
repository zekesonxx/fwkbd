use std::process::{exit, Stdio};
use std::thread::{current, sleep};
use std::time::{Duration, Instant};

use keyframe::ease_with_scaled_time;
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::screensaver;

use keyframe::{ease, functions};

use anyhow::*;

fn set_backlight(level: u8) -> Result<()> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PowerLedState {
    Auto,
    Off
}

fn set_power_led(state: PowerLedState) -> Result<()> {
    let cmd = std::process::Command::new("ectool")
        .arg("led")
        .arg("power")
        .arg(match state {
            PowerLedState::Auto => "auto",
            PowerLedState::Off => "off",
        })
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    NotIdle
}

fn main() -> Result<()> {
    use State::*;
    let backlight = 60;
    let timeout = 2_000;
    let fade_out = 1f32;
    let fade_in = 0.5f32;

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    if conn.extension_information(screensaver::X11_EXTENSION_NAME)?.is_none() {
        eprintln!("ScreenSaver extension is not supported");
        exit(1);
    }
    let mut idle;
    let mut state = Idle;
    let mut current_backlight = backlight;
    loop {
        // get current idle timer
        idle = screensaver::query_info(&conn, screen.root)?.reply()?.ms_since_user_input;
        //println!("{:?}", idle);

        if idle > timeout && state == NotIdle {
            println!("now idle");
            // we are now becoming idle
            state = Idle;
            let start = Instant::now();
            let starting_backlight = current_backlight as f32;
            let mut remaining;
            let mut iteration_start;
            loop {
                iteration_start = Instant::now();
                remaining = fade_out - start.elapsed().as_secs_f32();
                if remaining <= 0.0 {
                    if current_backlight != 0 {
                        set_backlight(0)?;
                        current_backlight = 0;
                    }
                    break;
                }
                let tween = ease_with_scaled_time(functions::EaseOut, 0.0, starting_backlight, remaining, fade_out);
                let tween = tween as u8;
                if tween == current_backlight {
                    sleep(Duration::from_millis(10));
                    continue;
                }
                set_backlight(tween)?;
                current_backlight = tween;
                println!("tween={tween}, remaining={remaining}");
                // check if they're not idle anymore
                idle = screensaver::query_info(&conn, screen.root)?.reply()?.ms_since_user_input;
                if idle <= timeout {
                    println!("user no longer idle");
                    state = Idle;
                    break;
                }

                // sleep so we don't make a million ectool calls
                let Some(sleep_timer) = Duration::from_millis(50).checked_sub(iteration_start.elapsed()) else { continue; };
                sleep(sleep_timer);
            }
        }
        
        if state == Idle && idle <= timeout {
            println!("now not idle");
            // we are now becoming not idle
            state = NotIdle;

            let start = Instant::now();
            let starting_backlight = current_backlight as f32;
            let goal_backlight = backlight as f32;
            let mut remaining;
            let mut iteration_start;
            loop {
                iteration_start = Instant::now();
                remaining = fade_in - start.elapsed().as_secs_f32();
                if remaining <= 0.0 {
                    if current_backlight != backlight {
                        set_backlight(backlight)?;
                        current_backlight = backlight;
                    }
                    break;
                }
                let tween = ease_with_scaled_time(functions::EaseInQuad, goal_backlight, starting_backlight, remaining, fade_in);
                let tween = tween as u8;
                if tween == current_backlight {
                    sleep(Duration::from_millis(10));
                    continue;
                }
                set_backlight(tween)?;
                current_backlight = tween;
                println!("tween={tween}, remaining={remaining}");

                // sleep so we don't make a million ectool calls
                let Some(sleep_timer) = Duration::from_millis(50).checked_sub(iteration_start.elapsed()) else { continue; };
                sleep(sleep_timer);
            }
        }

        // sleep so we don't eat all the CPU
        sleep(Duration::from_millis(match state {
            Idle => 200,
            NotIdle => 500
        }));
    }
    Ok(())
}
