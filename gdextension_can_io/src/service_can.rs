//! Adapter for the versioned, multi-client win-can-utils service.
use crate::timestamps::Clock;
use crosscan::{CanInterface, can::CanFrame, protocol::*, service::ServiceClient};
use std::io;

pub struct ServiceCan {
    client: ServiceClient,
    hardware_clock: Clock,
}

impl CanInterface for ServiceCan {
    async fn open(name: &str) -> io::Result<Self> {
        Ok(Self {
            client: ServiceClient::connect(name, Subscription::default()).await?,
            hardware_clock: Clock::default(),
        })
    }

    async fn get_bitrate(&mut self) -> io::Result<Option<u32>> {
        match self.client.request(Command::Status).await? {
            Reply::Status { status } if status.state == InterfaceState::Up => {
                Ok(Some(status.bitrate))
            }
            Reply::Status { status } => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                status
                    .last_error
                    .unwrap_or_else(|| format!("Interface is {:?}", status.state)),
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected service status",
            )),
        }
    }

    async fn read_frame(&mut self) -> io::Result<CanFrame> {
        // next_event uses a channel receiver, so the UI's read timeout is cancellation safe.
        let event = self.client.next_event().await?;
        let mut frame = event.frame.to_can()?;
        frame.set_timestamp(Some(
            self.hardware_clock
                .hardware_timestamp(event.hardware_timestamp_us, event.timestamp_us),
        ));
        Ok(frame)
    }

    async fn write_frame(&mut self, frame: CanFrame) -> io::Result<()> {
        match self
            .client
            .request(Command::Send {
                frame: Frame::from(&frame),
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected send acknowledgement",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncWriteExt, net::windows::named_pipe::ServerOptions};

    #[tokio::test]
    async fn service_status_send_and_cancelled_frame_read() {
        let name = format!("cydervis_test_{}", std::process::id());
        let mut pipe = ServerOptions::new()
            .first_pipe_instance(true)
            .create(pipe_name(&name).unwrap())
            .unwrap();
        let peer = tokio::spawn(async move {
            pipe.connect().await.unwrap();
            let status = Status {
                name: "test".into(),
                driver: "virtual".into(),
                device: "auto".into(),
                bitrate: 500_000,
                desired_up: true,
                state: InterfaceState::Up,
                bus_state: BusState::ErrorActive,
                timestamps: true,
                capabilities: Capabilities::default(),
                counters: Counters::default(),
                active_clients: 1,
                last_error: None,
                last_rx_timestamp_us: None,
                hardware_rx_error_counter: None,
                hardware_tx_error_counter: None,
            };
            for _ in 0..3 {
                let request: Request = read_message(&mut pipe).await.unwrap();
                let reply = match request.command {
                    Command::Hello { subscription } => {
                        assert!(subscription.receive);
                        Reply::Hello {
                            client_id: 1,
                            status: status.clone(),
                        }
                    }
                    Command::Status => Reply::Status {
                        status: status.clone(),
                    },
                    Command::Send { frame } => {
                        assert_eq!(frame.id, 0x123);
                        assert_eq!(frame.data, [1, 2, 3]);
                        Reply::Ok
                    }
                    _ => panic!("Unexpected command"),
                };
                write_message(
                    &mut pipe,
                    &ServerMessage {
                        version: VERSION,
                        body: ServerBody::Reply {
                            id: request.id,
                            reply,
                        },
                    },
                )
                .await
                .unwrap();
            }
            let message = ServerMessage {
                version: VERSION,
                body: ServerBody::Frame {
                    event: FrameEvent {
                        sequence: 1,
                        frame: Frame::from(&CanFrame::new(0x456, &[4, 5]).unwrap()),
                        direction: Direction::Rx,
                        source_client_id: None,
                        timestamp_us: 123_456,
                        hardware_timestamp_us: None,
                    },
                },
            };
            let bytes = serde_json::to_vec(&message).unwrap();
            pipe.write_u32_le(bytes.len() as u32).await.unwrap();
            // Deliver the payload after the caller has cancelled a read.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            pipe.write_all(&bytes).await.unwrap();
            pipe.flush().await.unwrap();
            // Host delivery times deliberately disagree with the hardware
            // spacing, including a valid zero counter and a host-only packet.
            for (index, (hardware, host)) in [
                (Some(0), 200_000),
                (Some(1000), 999_999),
                (Some(2000), 1_999_999),
                (None, 202_500),
                (Some(3000), 2_999_999),
            ]
            .into_iter()
            .enumerate()
            {
                let mut next = message.clone();
                if let ServerBody::Frame { event } = &mut next.body {
                    event.sequence = index as u64 + 2;
                    event.timestamp_us = host;
                    event.hardware_timestamp_us = hardware;
                }
                write_message(&mut pipe, &next).await.unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        });
        let mut client = ServiceCan::open(&name).await.unwrap();
        assert_eq!(client.get_bitrate().await.unwrap(), Some(500_000));
        client
            .write_frame(CanFrame::new(0x123, &[1, 2, 3]).unwrap())
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), client.read_frame())
                .await
                .is_err()
        );
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), client.read_frame())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(frame.id(), 0x456);
        assert_eq!(frame.data(), [4, 5]);
        assert_eq!(frame.timestamp(), Some(123_456));
        for expected in [200_000, 201_000, 202_000, 202_500, 203_000] {
            assert_eq!(
                client.read_frame().await.unwrap().timestamp(),
                Some(expected)
            );
        }
        peer.await.unwrap();
    }
}
