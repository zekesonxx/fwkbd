use std::process::{exit, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::screensaver;

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
    let step_up = 4;
    let step_down = 5;
    let step_down_mult = 5;
    let timeout = 4_000;

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
            for i in (0..current_backlight+1).step_by(step_down).rev() {
                // set the backlight
                let start = Instant::now();
                set_backlight(i)?;
                current_backlight = i;
                // check if we're not idle anymore
                idle = screensaver::query_info(&conn, screen.root)?.reply()?.ms_since_user_input;
                if idle < timeout {
                    state = NotIdle;
                    println!("early breaking from idle step_down");
                    break;
                }
                let timer = start.elapsed();
                println!("took {} ms", timer.as_millis());
                if timer.as_millis() < 9*step_down_mult {
                    sleep(Duration::from_millis(10*(step_down_mult as u64))-timer);
                }
                //sleep(Duration::from_millis(10));
            }
            //set_power_led(PowerLedState::Off)?;
        } else if state == Idle && idle <= timeout {
            println!("now not idle");
            // we are now becoming not idle
            state = NotIdle;
            //set_power_led(PowerLedState::Auto)?;
            for i in (current_backlight..backlight+1).step_by(step_up) {
                // set the backlight
                set_backlight(i)?;
                current_backlight = i;
                sleep(Duration::from_millis(1));
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
