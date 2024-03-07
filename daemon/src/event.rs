use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use krata::control::{GuestDestroyedEvent, GuestExitedEvent, GuestLaunchedEvent};
use log::error;
use tokio::{sync::broadcast, task::JoinHandle, time};
use uuid::Uuid;

use kratart::{GuestInfo, Runtime};

pub type DaemonEvent = krata::control::watch_events_reply::Event;

const EVENT_CHANNEL_QUEUE_LEN: usize = 1000;

#[derive(Clone)]
pub struct DaemonEventContext {
    sender: broadcast::Sender<DaemonEvent>,
}

impl DaemonEventContext {
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.sender.subscribe()
    }
}

pub struct DaemonEventGenerator {
    runtime: Runtime,
    last: HashMap<Uuid, GuestInfo>,
    sender: broadcast::Sender<DaemonEvent>,
}

impl DaemonEventGenerator {
    pub async fn new(runtime: Runtime) -> Result<(DaemonEventContext, DaemonEventGenerator)> {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_QUEUE_LEN);
        let generator = DaemonEventGenerator {
            runtime,
            last: HashMap::new(),
            sender: sender.clone(),
        };
        let context = DaemonEventContext { sender };
        Ok((context, generator))
    }

    async fn evaluate(&mut self) -> Result<()> {
        let guests = self.runtime.list().await?;
        let guests = {
            let mut map = HashMap::new();
            for guest in guests {
                map.insert(guest.uuid, guest);
            }
            map
        };

        let mut events: Vec<DaemonEvent> = Vec::new();

        for uuid in guests.keys() {
            if !self.last.contains_key(uuid) {
                events.push(DaemonEvent::GuestLaunched(GuestLaunchedEvent {
                    guest_id: uuid.to_string(),
                }));
            }
        }

        for uuid in self.last.keys() {
            if !guests.contains_key(uuid) {
                events.push(DaemonEvent::GuestDestroyed(GuestDestroyedEvent {
                    guest_id: uuid.to_string(),
                }));
            }
        }

        for (uuid, guest) in &guests {
            let Some(last) = self.last.get(uuid) else {
                continue;
            };

            if last.state.exit_code.is_some() {
                continue;
            }

            let Some(code) = guest.state.exit_code else {
                continue;
            };

            events.push(DaemonEvent::GuestExited(GuestExitedEvent {
                guest_id: uuid.to_string(),
                code,
            }));
        }

        self.last = guests;

        for event in events {
            let _ = self.sender.send(event);
        }

        Ok(())
    }

    pub async fn launch(mut self) -> Result<JoinHandle<()>> {
        Ok(tokio::task::spawn(async move {
            loop {
                if let Err(error) = self.evaluate().await {
                    error!("failed to evaluate daemon events: {}", error);
                    time::sleep(Duration::from_secs(5)).await;
                } else {
                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }))
    }
}
