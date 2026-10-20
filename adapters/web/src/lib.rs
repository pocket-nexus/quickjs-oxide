//! Browser implementation of engine host capabilities.
use js_sys::{Date, Math};
use wasm_bindgen::JsValue;

const TWO_TO_THE_32: f64 = 4_294_967_296.0;

/// The core's host over the browser's clock and time zone. The page that
/// embeds the engine shows results itself.
#[derive(Debug, Default)]
pub struct WebHost;

impl quickjs_oxide::engine::api::core::Host for WebHost {
    fn write(&mut self, _: quickjs_oxide::engine::api::core::Stream, _: &[u8]) {}

    fn now_ms(&mut self) -> f64 {
        Date::now()
    }

    fn local_tz_offset_minutes(&mut self, utc_ms: f64) -> i32 {
        let date = Date::new(&JsValue::from_f64((utc_ms as i64) as f64));
        let offset = date.get_timezone_offset();
        if offset.is_finite() { offset as i32 } else { 0 }
    }

    fn random_seed(&mut self) -> u64 {
        let high = (Math::random() * TWO_TO_THE_32) as u64;
        let low = (Math::random() * TWO_TO_THE_32) as u64;
        (high << 32) | low
    }

    fn read_file(&mut self, _: &str) -> Option<String> {
        None
    }
}
