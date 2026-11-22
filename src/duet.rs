//! Pair the independent K2 and Synthphone apps without sharing their audio engines.
use crate::duet_protocol::{TYPE, VERSION, valid_midi};
use js_sys::{Reflect, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{HtmlIFrameElement, MessageEvent, Url, Window};

const PHONE_URL: &str = "https://unitedcode.github.io/virtual-synthphone-e/";

fn phone_url() -> &'static str {
    let url = page();
    if url.protocol() == "http:"
        && matches!(url.hostname().as_str(), "127.0.0.1" | "localhost")
        && url.search_params().get("phone").as_deref() == Some("local")
    {
        "http://127.0.0.1:3032/"
    } else {
        PHONE_URL
    }
}

fn set(object: &JsValue, key: &str, value: impl Into<JsValue>) {
    let _ = Reflect::set(object, &key.into(), &value.into());
}
fn get(object: &JsValue, key: &str) -> JsValue {
    Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}
fn window() -> Window {
    web_sys::window().expect("browser window")
}
fn page() -> Url {
    Url::new(&window().location().href().unwrap_or_default()).expect("page URL")
}
pub fn embedded() -> bool {
    page().search_params().get("instrument").as_deref() == Some("synthphone")
        && window()
            .parent()
            .ok()
            .flatten()
            .is_some_and(|parent| parent != window())
}

struct Bridge {
    target: Window,
    origin: String,
    session: String,
    selected: bool,
    last_ready: f64,
    _message: Closure<dyn FnMut(MessageEvent)>,
}
thread_local! { static BRIDGE: RefCell<Option<Bridge>> = const { RefCell::new(None) }; }

fn envelope(kind: &str, session: &str) -> JsValue {
    let data: JsValue = js_sys::Object::new().into();
    set(&data, "type", TYPE);
    set(&data, "version", VERSION);
    set(&data, "kind", kind);
    set(&data, "session", session);
    data
}
fn matches(data: &JsValue, session: &str) -> bool {
    get(data, "type").as_string().as_deref() == Some(TYPE)
        && get(data, "version").as_f64() == Some(VERSION as f64)
        && get(data, "session").as_string().as_deref() == Some(session)
}

pub fn init() {
    if !embedded() || selected() {
        return;
    }
    let Ok(Some(parent)) = window().parent() else {
        return;
    };
    if parent == window() {
        return;
    }
    let origin = page().origin();
    let mut nonce = [0_u8; 16];
    if window()
        .crypto()
        .and_then(|c| c.get_random_values_with_u8_array(&mut nonce))
        .is_err()
    {
        return;
    }
    let session: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    let expected_parent = parent.clone();
    let expected_origin = origin.clone();
    let expected_session = session.clone();
    let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
        if event.origin() != expected_origin
            || event.source().as_ref().map(AsRef::<JsValue>::as_ref)
                != Some(expected_parent.as_ref())
        {
            return;
        }
        let data = event.data();
        if matches(&data, &expected_session)
            && get(&data, "kind").as_string().as_deref() == Some("ready")
        {
            BRIDGE.with_borrow_mut(|bridge| {
                if let Some(bridge) = bridge.as_mut() {
                    bridge.last_ready = js_sys::Date::now();
                }
            });
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    if window()
        .add_event_listener_with_callback("message", handler.as_ref().unchecked_ref())
        .is_err()
    {
        return;
    }
    BRIDGE.with_borrow_mut(|bridge| {
        *bridge = Some(Bridge {
            target: parent,
            origin,
            session,
            selected: true,
            last_ready: 0.0,
            _message: handler,
        })
    });
    heartbeat();
}

pub fn selected() -> bool {
    BRIDGE.with_borrow(|b| b.as_ref().is_some_and(|b| b.selected))
}
pub fn ready() -> bool {
    BRIDGE.with_borrow(|b| {
        b.as_ref().is_some_and(|b| {
            b.selected && b.last_ready > 0.0 && js_sys::Date::now() - b.last_ready < 3500.0
        })
    })
}
pub fn heartbeat() {
    BRIDGE.with_borrow(|b| {
        if let Some(b) = b.as_ref().filter(|b| b.selected) {
            let _ = b
                .target
                .post_message(&envelope("connect", &b.session), &b.origin);
        }
    });
}
pub fn send(bytes: &[u8]) -> bool {
    if !selected() {
        return false;
    }
    if valid_midi(bytes) && ready() {
        BRIDGE.with_borrow(|b| {
            if let Some(b) = b.as_ref() {
                let data = envelope("midi", &b.session);
                set(&data, "bytes", Uint8Array::from(bytes));
                let _ = b.target.post_message(&data, &b.origin);
            }
        });
    }
    true // Selected but not ready: discard stale notes, never fall back to double audio.
}
pub fn all_notes_off() {
    for channel in 0..16 {
        send(&[0xb0 | channel, 120, 0]);
        send(&[0xb0 | channel, 123, 0]);
    }
}
pub fn toggle() {
    all_notes_off();
    BRIDGE.with_borrow_mut(|b| {
        if let Some(b) = b.as_mut() {
            if b.selected {
                let _ = b
                    .target
                    .post_message(&envelope("disconnect", &b.session), &b.origin);
            }
            b.selected = !b.selected;
            b.last_ready = 0.0;
        }
    });
    heartbeat();
}
pub fn open() {
    let url = page();
    url.search_params().set("duet", "1");
    let _ = window().location().assign(&url.href());
}

/// The same small Rust entry builds the wrapper and K2, so Pages needs one
/// bundle/build. Each instrument keeps its existing independent deployment.
pub fn start_shell() -> bool {
    if page().search_params().get("duet").as_deref() != Some("1") {
        return false;
    }
    let document = window().document().expect("document");
    document.set_title("K2 + Synthphone E");
    let Some(body) = document.body() else {
        return true;
    };
    body.set_inner_html(include_str!("duet.html"));
    let url = page();
    url.search_params().delete("duet");
    url.search_params().set("instrument", "synthphone");
    let k2: HtmlIFrameElement = document.get_element_by_id("k2").unwrap().unchecked_into();
    let phone: HtmlIFrameElement = document
        .get_element_by_id("phone")
        .unwrap()
        .unchecked_into();
    k2.set_src(&url.href());
    let phone_url = phone_url();
    phone.set_src(phone_url);
    let back = document.get_element_by_id("back").unwrap();
    url.search_params().delete("instrument");
    let _ = back.set_attribute("href", &url.href());
    let phone_origin = Url::new(phone_url).expect("phone URL").origin();
    let local_origin = page().origin();
    let status = document.get_element_by_id("pair-status").unwrap();
    let mut session = String::new();
    let mut last_message = 0.0;
    let mut last_ready = 0.0;
    let relay = Closure::wrap(Box::new(move |event: MessageEvent| {
        let Some(source) = event.source() else {
            return;
        };
        let data = event.data();
        if get(&data, "type").as_string().as_deref() != Some(TYPE)
            || get(&data, "version").as_f64() != Some(VERSION as f64)
        {
            return;
        }
        let Some(kind) = get(&data, "kind").as_string() else {
            return;
        };
        let Some(id) = get(&data, "session")
            .as_string()
            .filter(|s| s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        else {
            return;
        };
        if event.origin() == local_origin
            && k2.content_window().as_ref().map(AsRef::<JsValue>::as_ref) == Some(source.as_ref())
        {
            if kind == "connect" {
                session = id.clone();
                if js_sys::Date::now() - last_ready > 3500.0 {
                    status.set_text_content(Some("Power on the phone to connect."));
                }
            }
            if id != session || !["connect", "disconnect", "midi"].contains(&kind.as_str()) {
                return;
            }
            if kind == "midi" {
                let bytes = get(&data, "bytes");
                if !bytes.is_instance_of::<Uint8Array>()
                    || !valid_midi(&Uint8Array::new(&bytes).to_vec())
                {
                    return;
                }
            }
            last_message = js_sys::Date::now();
            if let Some(target) = phone.content_window() {
                let _ = target.post_message(&data, &phone_origin);
            }
            if kind == "disconnect" {
                last_ready = 0.0;
                status
                    .set_text_content(Some("K2 synth selected. Reconnect with SYNTHPHONE in K2."));
            }
        } else if event.origin() == phone_origin
            && phone
                .content_window()
                .as_ref()
                .map(AsRef::<JsValue>::as_ref)
                == Some(source.as_ref())
            && id == session
            && kind == "ready"
        {
            if js_sys::Date::now() - last_message > 3500.0 {
                return;
            }
            last_ready = js_sys::Date::now();
            if let Some(target) = k2.content_window() {
                let _ = target.post_message(&data, &local_origin);
            }
            status.set_text_content(Some("Connected · K2 MIDI is playing the phone"));
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    let _ = window().add_event_listener_with_callback("message", relay.as_ref().unchecked_ref());
    relay.forget(); // Owns the listener for the lifetime of this wrapper document.
    true
}
