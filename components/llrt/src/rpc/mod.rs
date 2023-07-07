use anyhow::{bail, Result};
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, ffi::OsString};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
    sync::mpsc,
};

pub mod ui;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RpcMessageDirection {
    /// Serverbound (client-to-server) communication.
    #[serde(skip_serializing)]
    Serverbound(RpcServerboundMessage),

    /// Clientbound (server-to-client) communication.
    #[serde(skip_deserializing)]
    Clientbound(RpcClientboundMessage),
}

#[derive(Debug, PartialEq, Deserialize)]
pub enum RpcServerboundMessage {
    Ui(ui::UiRpcServerboundMessage),
}

#[derive(Debug, PartialEq, Serialize)]
pub enum RpcClientboundMessage {
    Ui(ui::UiRpcClientboundMessage),
}

pub struct RpcServerOptions {
    pub pipe_name: Cow<'static, str>,
    pub buffer_size: usize,
}

#[async_trait]
pub trait RpcServer {
    const SERVER_NAME: &'static str;

    type Serverbound: TryFrom<RpcServerboundMessage> + Send + 'static;
    type Clientbound: Into<RpcClientboundMessage> + Send + 'static;

    fn options(&self) -> &RpcServerOptions;

    /// Starts a task to listen on the named pipe.
    async fn listen_forever(&self) {
        loop {
            match self.await_connection().await {
                Ok(_) => info!("[rpc:{}] connection closed", Self::SERVER_NAME),
                Err(e) => error!("[rpc:{}] connection failed: {}", Self::SERVER_NAME, e),
            }
        }
    }

    async fn await_connection(&self) -> Result<()> {
        loop {
            info!(
                "[rpc:{}] awaiting connection on {}",
                Self::SERVER_NAME,
                self.options().pipe_name
            );

            let mut server = ServerOptions::new()
                .pipe_mode(PipeMode::Byte)
                .in_buffer_size(self.options().buffer_size as u32)
                .out_buffer_size(self.options().buffer_size as u32)
                .create(OsString::from(self.options().pipe_name.to_string()))?;

            server.connect().await?;
            self.handle_connection(&mut server).await?;
        }
    }

    async fn handle_connection(&self, server: &mut NamedPipeServer) -> Result<()> {
        let mut buf = BytesMut::with_capacity(self.options().buffer_size);
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<Self::Clientbound>();

        loop {
            tokio::select! {
                send_queue = send_rx.recv() => if let Some(outbound_msg) = send_queue {
                    // serialize the message
                    let mut message = Vec::new();
                    let mut serializer = rmp_serde::Serializer::new(&mut message);
                    RpcMessageDirection::Clientbound(<Self::Clientbound as Into<RpcClientboundMessage>>::into(outbound_msg))
                        .serialize(&mut serializer)?;

                    // write it
                    server.write_u32_le(message.len() as u32).await?;
                    server.write_all(&message).await?;
                },
                read = Self::triage_message(&mut buf, server) => match read {
                    Ok(message) => {
                        let cloned_tx = send_tx.clone();
                        tokio::spawn(async move {
                            match Self::dispatch_message(message, cloned_tx) {
                                Ok(_) => {},
                                Err(e) => error!("[rpc:{}] error dispatching message: {}", Self::SERVER_NAME, e),
                            }
                        });
                    }
                    Err(e) => bail!(e),
                }
            }
        }
    }

    async fn triage_message(
        mut buf: &mut BytesMut,
        reader: &mut (impl AsyncReadExt + Send + Unpin),
    ) -> Result<BytesMut> {
        let mut pending_len: Option<usize> = None;

        loop {
            match reader.read_buf(&mut buf).await {
                Ok(0) => bail!("pipe broken"),
                Ok(_) => {
                    // first check to see if this is a new message
                    if let None = pending_len {
                        // starting a new message, read the length
                        let _ = pending_len.insert(buf.split_to(4).get_u32_le() as usize);
                    }

                    // if we have a pending message length, check to see if we have enough data
                    if let Some(required) = pending_len {
                        if buf.len() >= required {
                            // split off the message, process it, and get ready for the next one
                            let message = buf.split_to(required);
                            assert_eq!(message.len(), required);
                            pending_len.take();

                            return Ok(message);
                        }
                    }
                }
                Err(e) => bail!(e),
            }
        }
    }

    fn dispatch_message(
        mut message: BytesMut,
        send_tx: mpsc::UnboundedSender<<Self as RpcServer>::Clientbound>,
    ) -> Result<()> {
        // optimization: if the first byte is < 0xDE or > 0xDF, then we know it's not a valid
        // msgpack structure for our purposes (since we only use maps), so we can skip the
        // deserialization step and pass it directly to process_incoming_message_raw
        // most stuff shouldn't use this, but it's useful for the UI server, where
        // performance is more important
        if message[0] < 0xDE || message[0] > 0xDF {
            if let Err(e) = <Self as RpcServer>::process_incoming_message_raw(send_tx, message) {
                error!(
                    "[rpc:{}] error processing message: {}",
                    Self::SERVER_NAME,
                    e
                );
            }

            return Ok(());
        }

        let mut de = rmp_serde::Deserializer::from_read_ref(&mut message[..]);
        match RpcMessageDirection::deserialize(&mut de) {
            Ok(rpc_message) => match rpc_message {
                RpcMessageDirection::Serverbound(msg) => match Self::Serverbound::try_from(msg) {
                    Ok(msg) => {
                        if let Err(e) = <Self as RpcServer>::process_incoming_message(send_tx, msg)
                        {
                            error!(
                                "[rpc:{}] error processing message: {}",
                                Self::SERVER_NAME,
                                e
                            );
                        }

                        Ok(())
                    }
                    Err(_) => {
                        bail!("inbound message was not of the correct type")
                    }
                },
                RpcMessageDirection::Clientbound(_) => {
                    bail!("received clientbound message on server pipe");
                }
            },
            Err(e) => bail!(e),
        }
    }

    async fn queue_send(&self, _message: Self::Clientbound) -> Result<()> {
        todo!()
    }

    fn process_incoming_message_raw(
        _send: mpsc::UnboundedSender<<Self as RpcServer>::Clientbound>,
        _message: BytesMut,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "process_incoming_message_raw is not implemented for this server"
        ))
    }

    fn process_incoming_message(
        send: mpsc::UnboundedSender<<Self as RpcServer>::Clientbound>,
        message: Self::Serverbound,
    ) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use rmp_serde::Deserializer;
    use serde::Deserialize;

    use super::*;

    const TEST_MESSAGE: &'static [u8] = &[
        0x40, 0x00, 0x00, 0x00, 0xDE, 0x00, 0x01, 0xA2, 0x55, 0x69, 0xDE, 0x00, 0x01, 0xA5, 0x50,
        0x61, 0x69, 0x6E, 0x74, 0xDE, 0x00, 0x08, 0xA2, 0x76, 0x77, 0x7B, 0xA2, 0x76, 0x68, 0xCD,
        0x01, 0xC8, 0xA1, 0x66, 0xA5, 0x52, 0x47, 0x42, 0x41, 0x38, 0xA2, 0x64, 0x78, 0x45, 0xA2,
        0x64, 0x79, 0x2A, 0xA2, 0x64, 0x77, 0xCD, 0x05, 0x39, 0xA2, 0x64, 0x68, 0xCD, 0x01, 0xA4,
        0xA1, 0x64, 0xC4, 0x04, 0x0C, 0x22, 0x38, 0x4E,
    ];

    async fn do_test_triage() -> BytesMut {
        let mut data = TEST_MESSAGE.clone();
        let mut buffer = BytesMut::new();

        let triaged = <ui::UiRpcServer as RpcServer>::triage_message(&mut buffer, &mut data).await;

        assert!(triaged.is_ok());
        triaged.unwrap()
    }

    #[tokio::test]
    async fn test_triage() {
        let triaged = do_test_triage().await;
        assert_eq!(triaged.len(), TEST_MESSAGE.len() - 4);
    }

    #[tokio::test]
    async fn test_decode() {
        let triaged = do_test_triage().await;
        let triaged_vec = triaged.to_vec();
        let mut de = Deserializer::new(triaged_vec.as_slice());
        let decoded = RpcMessageDirection::deserialize(&mut de); //rmp_serde::from_slice::<RpcMessageDirection>(&triaged);
        assert!(decoded.is_ok());
        let decoded = decoded.unwrap();

        assert!(matches!(decoded, RpcMessageDirection::Serverbound(_)));
    }
}
