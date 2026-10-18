#[cfg(not(target_arch = "wasm32"))]
pub struct Platform;

#[cfg(not(target_arch = "wasm32"))]
impl Platform {
    pub fn new(_: &eframe::egui::Context) -> Self {
        Self
    }
    pub fn fragment(&self) -> String {
        String::new()
    }
    pub fn changed_fragment(&mut self) -> Option<String> {
        None
    }
    pub fn save(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn language(&self) -> Option<String> {
        None
    }
    pub fn save_language(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::Platform;

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    pub struct Platform {
        last: String,
        listener: Closure<dyn FnMut(web_sys::Event)>,
    }

    impl Platform {
        pub fn new(ctx: &eframe::egui::Context) -> Self {
            let ctx = ctx.clone();
            let listener = Closure::wrap(
                Box::new(move |_: web_sys::Event| ctx.request_repaint()) as Box<dyn FnMut(_)>
            );
            let window = web_sys::window().expect("browser window");
            window
                .add_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref())
                .expect("hashchange listener");
            Self {
                last: window.location().hash().unwrap_or_default(),
                listener,
            }
        }

        pub fn fragment(&self) -> String {
            web_sys::window()
                .and_then(|window| window.location().hash().ok())
                .unwrap_or_default()
        }

        pub fn language(&self) -> Option<String> {
            let href = web_sys::window()?.location().href().ok()?;
            web_sys::Url::new(&href).ok()?.search_params().get("lang")
        }

        pub fn save_language(&self, code: &str) -> Result<(), String> {
            let window = web_sys::window().ok_or("browser window is unavailable")?;
            let href = window
                .location()
                .href()
                .map_err(|error| format!("{error:?}"))?;
            let url = web_sys::Url::new(&href).map_err(|error| format!("{error:?}"))?;
            url.search_params().set("lang", code);
            window
                .history()
                .map_err(|error| format!("{error:?}"))?
                .replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
                .map_err(|error| format!("{error:?}"))
        }

        pub fn changed_fragment(&mut self) -> Option<String> {
            let fragment = self.fragment();
            if fragment == self.last {
                return None;
            }
            self.last = fragment.clone();
            Some(fragment)
        }

        pub fn save(&mut self, fragment: &str) -> Result<(), String> {
            let window = web_sys::window().ok_or("browser window is unavailable")?;
            let hash = format!("#{fragment}");
            window
                .history()
                .map_err(|error| format!("{error:?}"))?
                .replace_state_with_url(&JsValue::NULL, "", Some(&hash))
                .map_err(|error| format!("{error:?}"))?;
            // replaceState does not fire hashchange; remember our own writes.
            self.last = hash;
            Ok(())
        }
    }

    impl Drop for Platform {
        fn drop(&mut self) {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "hashchange",
                    self.listener.as_ref().unchecked_ref(),
                );
            }
        }
    }
}
