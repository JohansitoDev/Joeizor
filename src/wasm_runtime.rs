use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use wasm_bindgen::prelude::*;
use web_sys::{Element, Event, Window};

struct SignalInner {
    value: JsValue,
    subscribers: Vec<Weak<RefCell<EffectInner>>>,
}

struct EffectInner {
    callback: js_sys::Function,
    dependencies: Vec<Weak<RefCell<SignalInner>>>,
    children: Vec<Rc<RefCell<EffectInner>>>,
}

thread_local! {
    static SIGNALS: RefCell<HashMap<String, Signal>> = RefCell::new(HashMap::new());
    static EFFECTS: RefCell<HashMap<String, Vec<Rc<RefCell<EffectInner>>>>> = RefCell::new(HashMap::new());
    static CURRENT_SCOPE: RefCell<Option<String>> = const { RefCell::new(None) };
    static ACTIVE_EFFECT: RefCell<Option<Rc<RefCell<EffectInner>>>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct Signal {
    inner: Rc<RefCell<SignalInner>>,
}

#[wasm_bindgen]
impl Signal {
    pub fn get(&self) -> JsValue {
        ACTIVE_EFFECT.with(|active| {
            if let Some(effect) = active.borrow().as_ref() {
                self.track(effect);
            }
        });
        self.inner.borrow().value.clone()
    }

    pub fn set(&self, value: JsValue) -> Result<(), JsValue> {
        let subscribers = {
            let mut inner = self.inner.borrow_mut();
            if js_sys::Object::is(&inner.value, &value) {
                return Ok(());
            }
            inner.value = value;
            inner
                .subscribers
                .retain(|subscriber| subscriber.strong_count() > 0);
            inner.subscribers.clone()
        };
        for subscriber in subscribers {
            if let Some(effect) = subscriber.upgrade() {
                run_effect(effect)?;
            }
        }
        Ok(())
    }
}

impl Signal {
    fn new(value: JsValue) -> Self {
        Self {
            inner: Rc::new(RefCell::new(SignalInner {
                value,
                subscribers: Vec::new(),
            })),
        }
    }

    fn track(&self, effect: &Rc<RefCell<EffectInner>>) {
        let mut inner = self.inner.borrow_mut();
        let exists = inner
            .subscribers
            .iter()
            .filter_map(Weak::upgrade)
            .any(|subscriber| Rc::ptr_eq(&subscriber, effect));
        if !exists {
            inner.subscribers.push(Rc::downgrade(effect));
        }
        let signal = Rc::downgrade(&self.inner);
        let mut effect_inner = effect.borrow_mut();
        if !effect_inner
            .dependencies
            .iter()
            .filter_map(Weak::upgrade)
            .any(|dependency| Rc::ptr_eq(&dependency, &self.inner))
        {
            effect_inner.dependencies.push(signal);
        }
    }
}

#[wasm_bindgen(js_name = state)]
pub fn state_signal(scope: &str, name: &str, initial: JsValue) -> Signal {
    let key = format!("{scope}\0{name}");
    SIGNALS.with(|signals| {
        let mut signals = signals.borrow_mut();
        signals
            .entry(key)
            .or_insert_with(|| Signal::new(initial))
            .clone()
    })
}

#[wasm_bindgen]
pub fn begin_render(scope: &str) {
    EFFECTS.with(|effects| {
        let mut effects = effects.borrow_mut();
        effects.clear();
        effects.insert(scope.to_owned(), Vec::new());
    });
    CURRENT_SCOPE.with(|current| *current.borrow_mut() = Some(scope.to_owned()));
}

#[wasm_bindgen]
pub fn effect(callback: js_sys::Function) -> Result<(), JsValue> {
    let effect = Rc::new(RefCell::new(EffectInner {
        callback,
        dependencies: Vec::new(),
        children: Vec::new(),
    }));
    let parent = ACTIVE_EFFECT.with(|active| active.borrow().clone());
    if let Some(parent) = parent {
        parent.borrow_mut().children.push(effect.clone());
    } else if let Some(scope) = CURRENT_SCOPE.with(|current| current.borrow().clone()) {
        EFFECTS.with(|effects| {
            effects
                .borrow_mut()
                .entry(scope)
                .or_default()
                .push(effect.clone())
        });
    }
    run_effect(effect)
}

fn run_effect(effect: Rc<RefCell<EffectInner>>) -> Result<(), JsValue> {
    let (dependencies, children) = {
        let mut inner = effect.borrow_mut();
        (
            std::mem::take(&mut inner.dependencies),
            std::mem::take(&mut inner.children),
        )
    };
    drop(children);
    for dependency in dependencies.into_iter().filter_map(|weak| weak.upgrade()) {
        dependency.borrow_mut().subscribers.retain(|subscriber| {
            subscriber
                .upgrade()
                .is_some_and(|candidate| !Rc::ptr_eq(&candidate, &effect))
        });
    }

    let callback = effect.borrow().callback.clone();
    let previous = ACTIVE_EFFECT.with(|active| active.replace(Some(effect)));
    let result = callback.call0(&JsValue::UNDEFINED).map(|_| ());
    ACTIVE_EFFECT.with(|active| *active.borrow_mut() = previous);
    result
}

#[wasm_bindgen]
pub fn get_element(id: &str) -> Result<Element, JsValue> {
    document()?
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("No existe el elemento #{id}")))
}

#[wasm_bindgen]
pub fn create_element(tag: &str) -> Result<Element, JsValue> {
    document()?.create_element(tag)
}

#[wasm_bindgen]
pub fn append(parent: &Element, child: &Element) -> Result<(), JsValue> {
    parent.append_child(child).map(|_| ())
}

#[wasm_bindgen]
pub fn clear_children(element: &Element) -> Result<(), JsValue> {
    while let Some(child) = element.first_child() {
        element.remove_child(&child)?;
    }
    Ok(())
}

#[wasm_bindgen]
pub fn set_text(element: &Element, text: &str) {
    element.set_text_content(Some(text));
}

#[wasm_bindgen]
pub fn set_class(element: &Element, class_name: &str) -> Result<(), JsValue> {
    element.set_attribute("class", class_name)
}

#[wasm_bindgen]
pub fn set_attribute(element: &Element, name: &str, value: &str) -> Result<(), JsValue> {
    element.set_attribute(name, value)
}

#[wasm_bindgen]
pub fn listen(element: &Element, name: &str, callback: &js_sys::Function) -> Result<(), JsValue> {
    element.add_event_listener_with_callback(name, callback)
}

#[wasm_bindgen]
pub fn navigate(path: &str) -> Result<(), JsValue> {
    let window = window()?;
    window
        .history()?
        .push_state_with_url(&JsValue::NULL, "", Some(path))?;
    window.dispatch_event(&Event::new("popstate")?)?;
    Ok(())
}

fn document() -> Result<web_sys::Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("El documento del navegador no esta disponible"))
}

fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("Window no esta disponible"))
}
