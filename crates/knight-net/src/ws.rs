//! WebSocket transport: tungstenite on a background thread natively, the browser's WebSocket on
//! the web. Both speak the relay protocol in binary frames.

use crate::relay::{ClientMsg, ServerMsg};
use crate::transport::{ConnState, Transport};

#[cfg(not(target_arch = "wasm32"))]
pub use native::WsTransport;
#[cfg(target_arch = "wasm32")]
pub use web::WsTransport;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use crate::codec::Wire;
    use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tungstenite::{Message, stream::MaybeTlsStream};

    pub struct WsTransport {
        out: Sender<ClientMsg>,
        inbox: Receiver<ServerMsg>,
        state: Arc<Mutex<(ConnState, Option<String>)>>,
    }

    impl WsTransport {
        pub fn open(url: &str, join: ClientMsg) -> Self {
            let (out_tx, out_rx) = channel::<ClientMsg>();
            let (in_tx, in_rx) = channel::<ServerMsg>();
            let state = Arc::new(Mutex::new((ConnState::Connecting, None)));
            let st = state.clone();
            let url = url.to_string();
            std::thread::spawn(move || {
                let fail = |e: String| {
                    log::warn!("knight-net: {e}");
                    *st.lock().unwrap() = (ConnState::Closed, Some(e));
                };
                let (mut ws, _) = match tungstenite::connect(&url) {
                    Ok(c) => c,
                    Err(e) => return fail(format!("connect {url}: {e}")),
                };
                if let MaybeTlsStream::Plain(s) = ws.get_mut() {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(10)));
                    let _ = s.set_nodelay(true);
                }
                if let Err(e) = ws.send(Message::Binary(join.to_bytes().into())) {
                    return fail(e.to_string());
                }
                st.lock().unwrap().0 = ConnState::Connected;
                loop {
                    loop {
                        match out_rx.try_recv() {
                            Ok(m) => {
                                if let Err(e) = ws.send(Message::Binary(m.to_bytes().into())) {
                                    return fail(e.to_string());
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            // The transport was dropped: close politely.
                            Err(TryRecvError::Disconnected) => {
                                let _ = ws.close(None);
                                return;
                            }
                        }
                    }
                    match ws.read() {
                        Ok(Message::Binary(b)) => match ServerMsg::from_bytes(&b) {
                            Ok(m) => {
                                if in_tx.send(m).is_err() {
                                    return;
                                }
                            }
                            Err(e) => log::warn!("knight-net: bad message: {e}"),
                        },
                        Ok(Message::Close(_)) => return fail("closed by relay".into()),
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(e))
                            if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                        Err(e) => return fail(e.to_string()),
                    }
                }
            });
            WsTransport { out: out_tx, inbox: in_rx, state }
        }
    }

    impl Transport for WsTransport {
        fn send_raw(&mut self, msg: ClientMsg) {
            let _ = self.out.send(msg);
        }

        fn poll_raw(&mut self) -> Vec<ServerMsg> {
            self.inbox.try_iter().collect()
        }

        fn state(&self) -> ConnState {
            self.state.lock().unwrap().0
        }

        fn error(&self) -> Option<String> {
            self.state.lock().unwrap().1.clone()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use crate::codec::Wire;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    #[derive(Default)]
    struct Shared {
        inbox: VecDeque<ServerMsg>,
        pending: Vec<ClientMsg>,
        state: Option<ConnState>,
        error: Option<String>,
    }

    pub struct WsTransport {
        ws: Option<WebSocket>,
        shared: Rc<RefCell<Shared>>,
        _handlers: Vec<Closure<dyn FnMut(JsValue)>>,
    }

    fn send(ws: &WebSocket, m: &ClientMsg) {
        let _ = ws.send_with_u8_array(&m.to_bytes());
    }

    impl WsTransport {
        pub fn open(url: &str, join: ClientMsg) -> Self {
            let shared = Rc::new(RefCell::new(Shared { state: Some(ConnState::Connecting), ..Default::default() }));
            let ws = match WebSocket::new(url) {
                Ok(ws) => ws,
                Err(e) => {
                    let mut s = shared.borrow_mut();
                    s.state = Some(ConnState::Closed);
                    s.error = Some(format!("{e:?}"));
                    drop(s);
                    return WsTransport { ws: None, shared, _handlers: Vec::new() };
                }
            };
            ws.set_binary_type(BinaryType::Arraybuffer);
            let mut handlers = Vec::new();
            {
                let sh = shared.clone();
                let w = ws.clone();
                let on_open = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    send(&w, &join);
                    let mut s = sh.borrow_mut();
                    for m in s.pending.drain(..) {
                        send(&w, &m);
                    }
                    s.state = Some(ConnState::Connected);
                });
                ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
                handlers.push(on_open);
            }
            {
                let sh = shared.clone();
                let on_msg = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                    let ev: MessageEvent = ev.unchecked_into();
                    if let Ok(buf) = ev.data().dyn_into::<js_sys::ArrayBuffer>() {
                        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                        match ServerMsg::from_bytes(&bytes) {
                            Ok(m) => sh.borrow_mut().inbox.push_back(m),
                            Err(e) => log::warn!("knight-net: bad message: {e}"),
                        }
                    }
                });
                ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
                handlers.push(on_msg);
            }
            for set_close in [true, false] {
                let sh = shared.clone();
                let h = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    let mut s = sh.borrow_mut();
                    s.state = Some(ConnState::Closed);
                    s.error.get_or_insert_with(|| "could not reach the relay".into());
                });
                if set_close {
                    ws.set_onclose(Some(h.as_ref().unchecked_ref()));
                } else {
                    ws.set_onerror(Some(h.as_ref().unchecked_ref()));
                }
                handlers.push(h);
            }
            WsTransport { ws: Some(ws), shared, _handlers: handlers }
        }
    }

    impl Transport for WsTransport {
        fn send_raw(&mut self, msg: ClientMsg) {
            let connected = self.shared.borrow().state == Some(ConnState::Connected);
            match (&self.ws, connected) {
                (Some(ws), true) => send(ws, &msg),
                _ => self.shared.borrow_mut().pending.push(msg),
            }
        }

        fn poll_raw(&mut self) -> Vec<ServerMsg> {
            self.shared.borrow_mut().inbox.drain(..).collect()
        }

        fn state(&self) -> ConnState {
            self.shared.borrow().state.unwrap_or(ConnState::Closed)
        }

        fn error(&self) -> Option<String> {
            self.shared.borrow().error.clone()
        }
    }

    impl Drop for WsTransport {
        fn drop(&mut self) {
            if let Some(ws) = &self.ws {
                let _ = ws.close();
            }
        }
    }
}
