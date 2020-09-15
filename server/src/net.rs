use crate::error::Error;
use async_std::net::TcpStream;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures::prelude::*;
use futures_util::core_reexport::pin::Pin;
use futures_util::core_reexport::task::{Context, Poll};
use proto::prelude::*;
use async_tls::server::TlsStream;

pub type MaybeTlsStream = WebSocketStream<async_tungstenite::stream::Stream<TcpStream, TlsStream<TcpStream>>>;

pub struct NTSocket {
    sock: MaybeTlsStream,
}

impl NTSocket {
    pub fn new(sock: MaybeTlsStream) -> NTSocket {
        NTSocket { sock }
    }
}

impl Stream for NTSocket {
    type Item = Result<NTMessage, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match futures::ready!(Stream::poll_next(Pin::new(&mut self.sock), cx)) {
            Some(msg) => match msg {
                Ok(Message::Binary(blob)) => {
                    // Ignore any invalid messages in the data stream
                    Poll::Ready(Some(Ok(NTMessage::Binary(
                        NTBinaryMessage::from_slice(&blob[..])
                            .into_iter()
                            .filter_map(|msg| msg.ok())
                            .collect(),
                    ))))
                }
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<Vec<NTTextMessage>>(&text) {
                        Ok(msg) => Poll::Ready(Some(Ok(NTMessage::Text(msg)))),
                        Err(e) => Poll::Ready(Some(Err(Error::JSON(e)))),
                    }
                }
                Ok(Message::Close(_)) => Poll::Ready(Some(Ok(NTMessage::Close))),
                // Don't care about control frames for now
                Ok(_) => Poll::Pending,
                Err(e) => Poll::Ready(Some(Err(Error::Tungstenite(e)))),
            },
            None => Poll::Ready(None),
        }
    }
}

impl Sink<NTMessage> for NTSocket {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::poll_ready(Pin::new(&mut self.sock), cx).map_err(Into::into)
    }

    fn start_send(mut self: Pin<&mut Self>, item: NTMessage) -> Result<(), Self::Error> {
        match item {
            NTMessage::Text(msg) => {
                let frame = serde_json::to_string(&msg)?;
                Sink::start_send(Pin::new(&mut self.sock), Message::Text(frame)).map_err(Into::into)
            }
            NTMessage::Binary(msg) => {
                let frame = msg
                    .into_iter()
                    .map(|msg| rmp_serde::to_vec(&msg).unwrap())
                    .flatten()
                    .collect::<Vec<u8>>();
                Sink::start_send(Pin::new(&mut self.sock), Message::Binary(frame))
                    .map_err(Into::into)
            }
            NTMessage::Close => {
                Sink::start_send(Pin::new(&mut self.sock), Message::Close(None)).map_err(Into::into)
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::poll_flush(Pin::new(&mut self.sock), cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::poll_close(Pin::new(&mut self.sock), cx).map_err(Into::into)
    }
}
