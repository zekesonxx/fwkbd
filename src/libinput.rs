//! libinput wrapper code
use anyhow::{Result, anyhow};
use input::event::EventTrait;
use log::error;
use tokio::{sync::mpsc, task::JoinHandle};

use std::time::Instant;

use input::{Libinput, LibinputInterface};
use std::fs::{File, OpenOptions};
use std::os::unix::{fs::OpenOptionsExt, io::OwnedFd};
use std::path::Path;

use libc::{O_RDWR, O_WRONLY};


struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read(flags & O_RDWR != 0)
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        let _ = File::from(fd);
    }
}

#[derive(Clone, Debug)]
pub struct LibinputSyncEvent {
    // pub device_name: String,
    // pub sysname: String,
    // pub id_product: u32,
    // pub id_vendor: u32,
    pub event_type: LibinputSyncEventType,
    pub instant: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibinputSyncEventType {
    DeviceAdded,
    DeviceRemoved,
    Key,
    Gesture,
    Pointer,
    Unknown
}

impl From<&input::Event> for LibinputSyncEvent {
    fn from(e: &input::Event) -> Self {
        use input::Event::*;
        use LibinputSyncEventType as t;
        // let d = e.device();
        LibinputSyncEvent {
            instant: Instant::now(),
            // device_name: d.name().to_string(),
            // sysname: d.sysname().to_string(),
            // id_product: d.id_product(),
            // id_vendor: d.id_vendor(),
            event_type: match e {
                Device(input::event::DeviceEvent::Added(_)) => t::DeviceAdded,
                Device(input::event::DeviceEvent::Removed(_)) => t::DeviceRemoved,
                Keyboard(_) => t::Key,
                Gesture(_) => t::Gesture,
                Pointer(_) => t::Pointer,
                _ => t::Unknown
            }
        }
    }
}


pub struct LibinputEventListener {
    pub _handle: JoinHandle<()>,
    _rx: mpsc::Receiver<LibinputSyncEvent>,
}

impl LibinputEventListener {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<LibinputSyncEvent>(100);

        let blocking_task = tokio::task::spawn_blocking(move || {
            use rustix::event::{poll, PollFlags, PollFd};
            let mut input = Libinput::new_with_udev(Interface);
            let Ok(_) = input.udev_assign_seat("seat0") else {
                return;
            };
            while poll(&mut [PollFd::new(&input, PollFlags::IN)], -1).is_ok() {
                let Ok(_) = input.dispatch() else { return; };
                for ref event in &mut input {
                    // throw away events if we fill up the buffer
                    // so we don't anger libinput
                    if tx.capacity() > 1 {
                        let Ok(_) = tx.blocking_send(event.into()) else {
                            return;
                        };
                    }
                }
            }
            error!("libinput event listener died");
        });
        
        Self {
            _handle: blocking_task,
            _rx: rx
        }
    }

    /// Return `true` if no messages are currently queued up
    pub fn is_empty(&self) -> bool {
        self._rx.is_empty()
    }

    /// Wait for the next event and return it
    pub async fn next(&mut self) -> Result<LibinputSyncEvent> {
        self._rx.recv().await.ok_or_else(|| anyhow!("libinput event handler died"))
    }
}

impl Drop for LibinputEventListener {
    fn drop(&mut self) {
        self._handle.abort();
        self._rx.close();
    }
}