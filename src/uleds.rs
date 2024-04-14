//! uleds driver
//! 
//! creates a fake keyboard led driver
//! 
//! https://www.kernel.org/doc/html/latest/leds/uleds.html

use tokio::sync::Notify;
use tokio::task::JoinHandle;

use anyhow::Result;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};
use tokio::io::AsyncReadExt;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const LED_MAX_NAME_SIZE: usize = 64;

const DEVICE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "::kbd_backlight");
//const DEVICE_NAME: &str = concat!("tpacpi::kbd_backlight");

pub struct Uleds {
    _handle: JoinHandle<()>,
    _notify: Arc<Notify>,
    _brightness: Arc<AtomicU8>
}

impl Uleds {
    pub async fn new(starting_brightness: u8) -> Result<Uleds> {
        assert!(starting_brightness <= 100, "starting_brightness out of range");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uleds")
            .await?;
        let mut buf = DEVICE_NAME.as_bytes().to_vec();
        buf.resize(LED_MAX_NAME_SIZE, 0);
        #[cfg(target_endian = "little")]
        buf.push(100);
        buf.push(0);
        buf.push(0);
        buf.push(0);
        #[cfg(target_endian = "big")]
        buf.push(100);
        file.write_all(&buf).await?;
        file.flush().await?;

        // force our brightness to start at 100
        tokio::fs::write(format!("/sys/class/leds/{DEVICE_NAME}/brightness"),
            starting_brightness.to_string()).await?;

        // set up the reading buffer
        let brightness = Arc::new(AtomicU8::new(starting_brightness));
        let notify = Arc::new(Notify::new());
        let _brightness = brightness.clone();
        let _notify = notify.clone();

        
        // spawn a task to constantly read brightness updates
        let handle = tokio::spawn(async move {
            #[cfg(target_endian = "little")]
            while let Ok(i) = file.read_u32_le().await {
                brightness.store(i as u8, Ordering::Relaxed);
                notify.notify_waiters();
            }
            #[cfg(target_endian = "big")]
            while let Ok(i) = file.read_u32().await {
                brightness.store(i as u8, Ordering::Relaxed);
                notify.notify_waiters();
            }
        });

        
        Ok(Uleds {
            _handle: handle,
            _notify,
            _brightness
        })
    }

    pub async fn wait_for_update(&self) {
        self._notify.notified().await
    }

    pub fn brightness(&self) -> u8 {
        self._brightness.load(Ordering::Relaxed)
    }
}