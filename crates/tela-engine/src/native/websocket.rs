//! Bounded, context-owned WebSockets. See docs/WEBSOCKETS.md for the host seam.
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rquickjs::{Array, ArrayBuffer, Ctx, Exception, Function, Object, TypedArray, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    tungstenite::{
        client::IntoClientRequest,
        handshake::client::{Request, Response},
        http::{header, HeaderValue},
        protocol::WebSocketConfig,
        Message,
    },
    Connector,
};
use tokio_util::sync::CancellationToken;

pub const MAX_CONNECTIONS: usize = 8;
pub const MAX_PAYLOAD: usize = 1024 * 1024;
const QUEUE_CAP: usize = 8;
const DIAL_TIMEOUT: Duration = Duration::from_secs(15);
const IO_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSING: u8 = 2;
const CLOSED: u8 = 3;

/// Retains the old registry construction seam: Arc::new(Mutex::new(HashMap::new())).
pub type WsSenders = Arc<Mutex<HashMap<u64, Arc<WsConnection>>>>;

/// Host-only TLS configuration. Default uses normal public roots and validation.
/// Hosts may provide Connector::Rustls with additional test/private CA roots.
/// Plain connectors are rejected: this seam must not downgrade wss to cleartext.
#[derive(Clone, Default)]
pub struct WsConfig {
    pub connector: Option<Connector>,
}

#[derive(Debug)]
enum Event {
    Open(String),
    Text(String),
    Binary(Vec<u8>),
    Error(&'static str),
    Close,
}

enum Outgoing {
    Message(Message),
    Flush(u64),
}

enum FlushEvent {
    Ok(u64),
    Err(u64),
}

#[derive(Default)]
struct Inbound {
    events: VecDeque<Event>,
    // Both lists together are bounded: completed notifications keep their slot
    // until JS drains them, so a caller cannot create unbounded promises.
    flush_pending: VecDeque<u64>,
    flushes: VecDeque<FlushEvent>,
    wake_pending: bool,
    finished: bool,
}

pub struct WsConnection {
    outgoing: mpsc::Sender<Outgoing>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    incoming: Mutex<Inbound>,
    cancel: CancellationToken,
    state: AtomicU8,
    shutdown: AtomicBool,
}

impl WsConnection {
    fn new(
        outgoing: mpsc::Sender<Outgoing>,
        action_tx: mpsc::UnboundedSender<serde_json::Value>,
    ) -> Self {
        Self {
            outgoing,
            action_tx,
            incoming: Mutex::new(Inbound::default()),
            cancel: CancellationToken::new(),
            state: AtomicU8::new(CONNECTING),
            shutdown: AtomicBool::new(false),
        }
    }

    // Called with incoming locked, so enqueue + notification acknowledgement are atomic.
    fn wake(&self, id: u64, queue: &mut Inbound, tx: &mpsc::UnboundedSender<serde_json::Value>) {
        if !queue.wake_pending && !self.shutdown.load(Ordering::SeqCst) {
            queue.wake_pending = true;
            if tx
                .send(serde_json::json!({
                    "type": "__tela_ws__", "id": id, "event": "drain", "data": ""
                }))
                .is_err()
            {
                self.cancel.cancel();
            }
        }
    }

    fn enqueue(
        &self,
        id: u64,
        event: Event,
        tx: &mpsc::UnboundedSender<serde_json::Value>,
    ) -> bool {
        let mut queue = self.incoming.lock().unwrap();
        if queue.finished || self.shutdown.load(Ordering::SeqCst) || queue.events.len() >= QUEUE_CAP
        {
            return false;
        }
        queue.events.push_back(event);
        self.wake(id, &mut queue, tx);
        true
    }

    fn reject_flushes(&self, id: u64, tx: &mpsc::UnboundedSender<serde_json::Value>) {
        let mut queue = self.incoming.lock().unwrap();
        let mut rejected = false;
        while let Some(ticket) = queue.flush_pending.pop_front() {
            queue.flushes.push_back(FlushEvent::Err(ticket));
            rejected = true;
        }
        if rejected {
            self.wake(id, &mut queue, tx);
        }
    }

    fn finish(
        &self,
        id: u64,
        error: Option<&'static str>,
        tx: &mpsc::UnboundedSender<serde_json::Value>,
    ) {
        self.state.store(CLOSED, Ordering::SeqCst);
        self.reject_flushes(id, tx);
        let mut queue = self.incoming.lock().unwrap();
        if queue.finished {
            return;
        }
        queue.finished = true;
        if self.shutdown.load(Ordering::SeqCst) {
            queue.events.clear();
            return;
        }
        // Two reserved terminal slots: overflow cannot lose error/close or prevent cleanup.
        if let Some(error) = error {
            queue.events.push_back(Event::Error(error));
        }
        queue.events.push_back(Event::Close);
        self.wake(id, &mut queue, tx);
    }
}

/// Synchronously cancel all connecting/open tasks and discard queued native payloads.
/// Call on Engine shutdown AND Drop (including error exits). Safe to call repeatedly.
pub fn shutdown_ws(registry: &WsSenders) {
    let mut map = registry.lock().unwrap();
    let mut flushes = Vec::new();
    for (id, connection) in map.drain() {
        connection.state.store(CLOSED, Ordering::SeqCst);
        // Reject before marking shutdown: wake() suppresses ordinary shutdown
        // notifications, but a live context must not strand these promises.
        connection.reject_flushes(id, &connection.action_tx);
        connection.shutdown.store(true, Ordering::SeqCst);
        connection.cancel.cancel();
        let mut incoming = connection.incoming.lock().unwrap();
        incoming.events.clear();
        // Keep only undrained promise notifications reachable for one final pull.
        // They are still bounded and pull removes this finished registry entry.
        if !incoming.flushes.is_empty() {
            incoming.finished = true;
            flushes.push((id, connection.clone()));
        }
    }
    map.extend(flushes);
}

// Only JS native functions retain this guard, never network tasks. Their finalization
// is a fallback, not the shutdown entry point: QuickJS can retain a dropped context
// until runtime destruction. Hosts must call shutdown_ws when disposing an engine.
struct ContextOwner(WsSenders);
impl Drop for ContextOwner {
    fn drop(&mut self) {
        shutdown_ws(&self.0);
    }
}

fn request(url: &str, protocols: &[String], origin: Option<&str>) -> Result<Request, &'static str> {
    if url.len() > 8192 || protocols.len() > 32 {
        return Err("Invalid WebSocket constructor");
    }
    let url = reqwest::Url::parse(url).map_err(|_| "Invalid WebSocket URL")?;
    if !matches!(url.scheme(), "ws" | "wss")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid WebSocket URL");
    }
    for (i, protocol) in protocols.iter().enumerate() {
        if protocol.is_empty()
            || protocol.len() > 256
            || !protocol
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            || protocols[..i].contains(protocol)
        {
            return Err("Invalid WebSocket protocols");
        }
    }
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| "Invalid WebSocket URL")?;
    if !protocols.is_empty() {
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_str(&protocols.join(", "))
                .map_err(|_| "Invalid WebSocket protocols")?,
        );
    }
    if let Some(origin) = origin {
        if origin.len() > 2048 {
            return Err("Invalid WebSocket origin");
        }
        if origin != "null" {
            let parsed = reqwest::Url::parse(origin).map_err(|_| "Invalid WebSocket origin")?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.path() != "/"
                || parsed.origin().ascii_serialization() != origin
            {
                return Err("Invalid WebSocket origin");
            }
        }
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).map_err(|_| "Invalid WebSocket origin")?,
        );
    }
    Ok(request)
}

fn selected_protocol(response: &Response, offered: &[String]) -> Result<String, &'static str> {
    let mut values = response
        .headers()
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter();
    let Some(value) = values.next() else {
        return Ok(String::new());
    };
    let value = value
        .to_str()
        .map_err(|_| "Invalid WebSocket protocol selection")?;
    if values.next().is_some() || !offered.iter().any(|p| p == value) {
        return Err("Invalid WebSocket protocol selection");
    }
    Ok(value.to_owned())
}

async fn transport(
    id: u64,
    connection: &WsConnection,
    request: Request,
    protocols: Vec<String>,
    config: WsConfig,
    mut outgoing: mpsc::Receiver<Outgoing>,
    tx: &mpsc::UnboundedSender<serde_json::Value>,
) -> Result<(), &'static str> {
    let limits = WebSocketConfig::default()
        .max_message_size(Some(MAX_PAYLOAD))
        .max_frame_size(Some(MAX_PAYLOAD))
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_PAYLOAD + 1024);
    let (mut socket, response) = tokio::time::timeout(
        DIAL_TIMEOUT,
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(limits),
            false,
            config.connector,
        ),
    )
    .await
    .map_err(|_| "WebSocket connection timed out")?
    .map_err(|_| "WebSocket connection failed")?;
    let protocol = selected_protocol(&response, &protocols)?;
    if connection
        .state
        .compare_exchange(CONNECTING, OPEN, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }
    if !connection.enqueue(id, Event::Open(protocol), tx) {
        return Err("WebSocket receive queue full");
    }
    loop {
        tokio::select! {
            message = outgoing.recv() => {
                let Some(outgoing) = message else { return Ok(()); };
                match outgoing {
                    Outgoing::Message(message) => {
                        tokio::time::timeout(IO_TIMEOUT, socket.send(message)).await
                            .map_err(|_| "WebSocket send timed out")?
                            .map_err(|_| "WebSocket transport failed")?;
                    }
                    Outgoing::Flush(ticket) => {
                        let result = tokio::time::timeout(IO_TIMEOUT, socket.flush()).await
                            .map_err(|_| "WebSocket send timed out")?
                            .map_err(|_| "WebSocket transport failed");
                        let mut queue = connection.incoming.lock().unwrap();
                        if let Some(position) = queue.flush_pending.iter().position(|&id| id == ticket) {
                            queue.flush_pending.remove(position);
                            queue.flushes.push_back(if result.is_ok() {
                                FlushEvent::Ok(ticket)
                            } else {
                                FlushEvent::Err(ticket)
                            });
                            connection.wake(id, &mut queue, tx);
                        }
                        result?;
                    }
                }
            }
            message = socket.next() => {
                let event = match message {
                    Some(Ok(Message::Text(text))) => Event::Text(text.to_string()),
                    Some(Ok(Message::Binary(bytes))) => Event::Binary(bytes.to_vec()),
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Ok(_)) => {
                        // Flush tungstenite's automatic pong without an unbounded write wait.
                        tokio::time::timeout(IO_TIMEOUT, socket.flush()).await
                            .map_err(|_| "WebSocket send timed out")?
                            .map_err(|_| "WebSocket transport failed")?;
                        continue;
                    }
                    Some(Err(_)) => return Err("WebSocket transport failed"),
                };
                if !connection.enqueue(id, event, tx) { return Err("WebSocket receive queue full"); }
            }
        }
    }
}

fn start(
    registry: &WsSenders,
    id: u64,
    request: Request,
    protocols: Vec<String>,
    config: WsConfig,
    tx: mpsc::UnboundedSender<serde_json::Value>,
    handle: &tokio::runtime::Handle,
) -> Result<(), &'static str> {
    let mut map = registry.lock().unwrap();
    if id == 0 || id > 9_007_199_254_740_991 || map.contains_key(&id) {
        return Err("Invalid WebSocket id");
    }
    if map.len() >= MAX_CONNECTIONS {
        return Err("WebSocket connection limit reached");
    }
    let (outgoing, receiver) = mpsc::channel(QUEUE_CAP);
    let connection = Arc::new(WsConnection::new(outgoing, tx.clone()));
    map.insert(id, connection.clone());
    handle.spawn(async move {
        let error = tokio::select! {
            biased;
            _ = connection.cancel.cancelled() => None,
            result = transport(id, &connection, request, protocols, config, receiver, &tx) => result.err(),
        };
        // Dropping transport drops the single owned socket; no detached reader/writer.
        connection.finish(id, error, &tx);
    });
    Ok(())
}

fn payload<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Message> {
    if let Some(text) = value.as_string() {
        let text = text.to_string()?;
        if text.len() > MAX_PAYLOAD {
            return Err(Exception::throw_range(ctx, "WebSocket payload too large"));
        }
        return Ok(Message::Text(text.into()));
    }
    let buffer;
    let view;
    let bytes = if let Some(ab) = ArrayBuffer::from_value(value.clone()) {
        buffer = ab;
        buffer.as_bytes()
    } else if let Ok(array) = TypedArray::<u8>::from_value(value) {
        view = array;
        view.as_bytes() // rquickjs applies byteOffset and byteLength, not the backing buffer length.
    } else {
        return Err(Exception::throw_type(
            ctx,
            "WebSocket.send expects string, ArrayBuffer or Uint8Array",
        ));
    }
    .ok_or_else(|| Exception::throw_type(ctx, "Detached WebSocket buffer"))?;
    if bytes.len() > MAX_PAYLOAD {
        return Err(Exception::throw_range(ctx, "WebSocket payload too large"));
    }
    Ok(Message::Binary(bytes.to_vec().into()))
}

fn send<'js>(
    ctx: Ctx<'js>,
    registry: &WsSenders,
    id: u64,
    value: Value<'js>,
) -> rquickjs::Result<()> {
    let connection = registry
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| Exception::throw_type(&ctx, "WebSocket is not open"))?;
    if connection.state.load(Ordering::SeqCst) != OPEN {
        return Err(Exception::throw_type(&ctx, "WebSocket is not open"));
    }
    let message = payload(&ctx, value)?;
    connection
        .outgoing
        .try_send(Outgoing::Message(message))
        .map_err(|_| Exception::throw_range(&ctx, "WebSocket send queue full or closed"))
}

fn flush<'js>(ctx: Ctx<'js>, registry: &WsSenders, id: u64, ticket: u64) -> rquickjs::Result<()> {
    let connection = registry
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| Exception::throw_type(&ctx, "WebSocket is not open"))?;
    if connection.state.load(Ordering::SeqCst) != OPEN {
        return Err(Exception::throw_type(&ctx, "WebSocket is not open"));
    }
    {
        let mut queue = connection.incoming.lock().unwrap();
        if queue.flush_pending.len() + queue.flushes.len() >= QUEUE_CAP {
            return Err(Exception::throw_range(&ctx, "WebSocket flush queue full"));
        }
        queue.flush_pending.push_back(ticket);
    }
    if connection
        .outgoing
        .try_send(Outgoing::Flush(ticket))
        .is_err()
    {
        let mut queue = connection.incoming.lock().unwrap();
        if let Some(position) = queue
            .flush_pending
            .iter()
            .position(|&pending| pending == ticket)
        {
            queue.flush_pending.remove(position);
        }
        return Err(Exception::throw_range(
            &ctx,
            "WebSocket flush queue full or closed",
        ));
    }
    Ok(())
}

fn pull<'js>(ctx: Ctx<'js>, registry: &WsSenders, id: u64) -> rquickjs::Result<Array<'js>> {
    let (events, flushes) = {
        let mut map = registry.lock().unwrap();
        if let Some(connection) = map.get(&id).cloned() {
            let mut queue = connection.incoming.lock().unwrap();
            queue.wake_pending = false;
            let events = std::mem::take(&mut queue.events);
            let flushes = std::mem::take(&mut queue.flushes);
            if queue.finished {
                map.remove(&id);
            }
            (events, flushes)
        } else {
            (VecDeque::new(), VecDeque::new())
        }
    };
    let array = Array::new(ctx.clone())?;
    for (i, event) in events.into_iter().enumerate() {
        let object = Object::new(ctx.clone())?;
        match event {
            Event::Open(protocol) => {
                object.set("type", "open")?;
                object.set("data", protocol)?;
            }
            Event::Text(text) => {
                object.set("type", "message")?;
                object.set("data", text)?;
            }
            Event::Binary(bytes) => {
                object.set("type", "message")?;
                // rquickjs 0.9's Vec-backed finalizer is unsafe after JS detachment.
                // Let QuickJS own this allocation so transfer/detach remains safe.
                object.set("data", ArrayBuffer::new_copy(ctx.clone(), &bytes)?)?;
            }
            Event::Error(error) => {
                object.set("type", "error")?;
                object.set("data", error)?;
            }
            Event::Close => {
                object.set("type", "close")?;
            }
        }
        array.set(i, object)?;
    }
    for event in flushes {
        let object = Object::new(ctx.clone())?;
        match event {
            FlushEvent::Ok(ticket) => {
                object.set("type", "flush")?;
                object.set("data", ticket)?;
            }
            FlushEvent::Err(ticket) => {
                object.set("type", "flush-error")?;
                object.set("data", ticket)?;
            }
        }
        array.set(array.len(), object)?;
    }
    Ok(array)
}

pub fn register_websocket(
    ctx: &Ctx<'_>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    registry: WsSenders,
    handle: tokio::runtime::Handle,
) -> anyhow::Result<()> {
    register_websocket_with_config(ctx, action_tx, registry, handle, WsConfig::default())
}

pub fn register_websocket_with_config(
    ctx: &Ctx<'_>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    registry: WsSenders,
    handle: tokio::runtime::Handle,
    config: WsConfig,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !matches!(config.connector, Some(Connector::Plain)),
        "WebSocket TLS downgrade is not allowed"
    );
    let owner = Arc::new(ContextOwner(registry));
    let connect_owner = owner.clone();
    let connect_tx = action_tx.clone();
    ctx.globals().set(
        "__tela_ws_connect__",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>,
                  id: u64,
                  url: String,
                  protocols: String,
                  origin: Option<String>|
                  -> rquickjs::Result<()> {
                let protocols: Vec<String> = serde_json::from_str(&protocols)
                    .map_err(|_| Exception::throw_type(&ctx, "Invalid WebSocket protocols"))?;
                let request = request(&url, &protocols, origin.as_deref())
                    .map_err(|error| Exception::throw_type(&ctx, error))?;
                start(
                    &connect_owner.0,
                    id,
                    request,
                    protocols,
                    config.clone(),
                    connect_tx.clone(),
                    &handle,
                )
                .map_err(|error| Exception::throw_type(&ctx, error))
            },
        )?,
    )?;
    let send_owner = owner.clone();
    ctx.globals().set(
        "__tela_ws_send__",
        Function::new(ctx.clone(), move |ctx, id, value| {
            send(ctx, &send_owner.0, id, value)
        })?,
    )?;
    let flush_owner = owner.clone();
    ctx.globals().set(
        "__tela_ws_flush__",
        Function::new(ctx.clone(), move |ctx, id, ticket| {
            flush(ctx, &flush_owner.0, id, ticket)
        })?,
    )?;
    let pull_owner = owner.clone();
    ctx.globals().set(
        "__tela_ws_pull__",
        Function::new(ctx.clone(), move |ctx, id| pull(ctx, &pull_owner.0, id))?,
    )?;
    let close_tx = action_tx.clone();
    ctx.globals().set(
        "__tela_ws_close__",
        Function::new(ctx.clone(), move |id: u64| {
            if let Some(connection) = owner.0.lock().unwrap().get(&id) {
                if connection.state.load(Ordering::SeqCst) != CLOSED {
                    connection.state.store(CLOSING, Ordering::SeqCst);
                    connection.reject_flushes(id, &close_tx);
                    connection.cancel.cancel();
                }
            }
        })?,
    )?;
    ctx.eval::<(), _>(WS_BOOTSTRAP)
        .map_err(|_| anyhow::anyhow!("failed to register WebSocket bootstrap"))?;
    Ok(())
}

const WS_BOOTSTRAP: &str = r#"
(function() {
    const instances = new Map();
    let nextId = 1;
    const connect = globalThis.__tela_ws_connect__;
    const send = globalThis.__tela_ws_send__;
    const flush = globalThis.__tela_ws_flush__;
    const close = globalThis.__tela_ws_close__;
    let nextFlushId = 1;
    const pull = globalThis.__tela_ws_pull__;
    globalThis.__tela_ws_event__ = function(id, type, ignored) {
        if (type !== 'drain') return;
        const events = pull(id);
        const ws = instances.get(id);
        if (!ws) return;
        let thrown, hasThrown = false;
        for (const event of events) {
            let handler, data;
            if (event.type === 'open') {
                if (ws.readyState !== WebSocket.CONNECTING) continue;
                ws._state = WebSocket.OPEN;
                ws._protocol = event.data;
                handler = ws.onopen; data = {type: 'open', target: ws};
            } else if (event.type === 'message') {
                if (ws.readyState !== WebSocket.OPEN) continue;
                handler = ws.onmessage; data = {type: 'message', target: ws, data: event.data};
            } else if (event.type === 'flush' || event.type === 'flush-error') {
                const pending = ws._flushes.get(event.data);
                if (pending) {
                    ws._flushes.delete(event.data);
                    if (event.type === 'flush') pending.resolve();
                    else pending.reject(new TypeError('WebSocket flush failed'));
                }
                continue;
            } else if (event.type === 'error') {
                handler = ws.onerror; data = {type: 'error', target: ws, message: event.data};
            } else if (event.type === 'close') {
                ws._state = WebSocket.CLOSED;
                instances.delete(id);
                handler = ws.onclose; data = {type: 'close', target: ws};
            }
            // A throwing handler must not strand terminal events or native resources.
            try { if (typeof handler === 'function') handler.call(ws, data); }
            catch (error) { if (!hasThrown) { thrown = error; hasThrown = true; } }
        }
        if (hasThrown) throw thrown;
    };
    class WebSocket {
        constructor(url, protocols, options) {
            if (typeof url !== 'string') throw new TypeError('Invalid WebSocket URL');
            if (protocols === undefined) protocols = [];
            if (typeof protocols === 'string') protocols = [protocols];
            if (!Array.isArray(protocols) || protocols.some(p => typeof p !== 'string'))
                throw new TypeError('Invalid WebSocket protocols');
            let origin = null;
            if (options !== undefined) {
                if (options === null || typeof options !== 'object' || Array.isArray(options)
                    || Reflect.ownKeys(options).some(k => k !== 'origin'))
                    throw new TypeError('Invalid WebSocket options');
                if ('origin' in options) {
                    if (typeof options.origin !== 'string') throw new TypeError('Invalid WebSocket origin');
                    origin = options.origin;
                }
            }
            const id = nextId++;
            connect(id, url, JSON.stringify(protocols), origin);
            this._id = id;
            this._state = WebSocket.CONNECTING;
            this._protocol = '';
            this._flushes = new Map();
            Object.defineProperty(this, 'url', {value: url, enumerable: true});
            this.onopen = this.onmessage = this.onerror = this.onclose = null;
            instances.set(id, this);
        }
        get readyState() { return this._state; }
        get protocol() { return this._protocol; }
        get binaryType() { return 'arraybuffer'; }
        set binaryType(value) {
            if (value !== 'arraybuffer') throw new TypeError('Only arraybuffer binaryType is supported');
        }
        send(data) {
            if (this.readyState !== WebSocket.OPEN) throw new TypeError('WebSocket is not open');
            send(this._id, data);
        }
        flush() {
            return new Promise((resolve, reject) => {
                const ticket = nextFlushId++;
                try {
                    flush(this._id, ticket);
                    this._flushes.set(ticket, {resolve, reject});
                } catch (error) { reject(error); }
            });
        }
        close() {
            if (this.readyState >= WebSocket.CLOSING) return;
            this._state = WebSocket.CLOSING;
            close(this._id);
        }
    }
    for (const [name, value] of [['CONNECTING',0], ['OPEN',1], ['CLOSING',2], ['CLOSED',3]]) {
        Object.defineProperty(WebSocket, name, {value});
        Object.defineProperty(WebSocket.prototype, name, {value});
    }
    globalThis.WebSocket = WebSocket;
})();
"#;

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod tests;
