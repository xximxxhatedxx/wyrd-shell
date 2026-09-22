use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{export_wasm_module, log_info, publish, send_update, WasmModule};

#[derive(Default)]
pub struct CounterModule {
    count: i64,
    step: i64,
}

impl WasmModule for CounterModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Counter WASM module initialized!");
        self.step = config.get("step").and_then(|v| v.as_i64()).unwrap_or(1);
        self.count = config.get("initial").and_then(|v| v.as_i64()).unwrap_or(0);
        self.render();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        log_info(&format!(
            "Counter received event: widget={}, event={}",
            widget_id, event
        ));
        if event == "click" || event == "press" {
            self.count += self.step;
            self.render();
            let _ = publish("counter.value", &json!(self.count));
        }
    }

    fn on_topic(&mut self, topic: &str, value: &Value) {
        log_info(&format!("Counter received topic {}: {:?}", topic, value));
    }

    fn tick(&mut self) {
        // Periodic background tick
    }

    fn shutdown(&mut self) {
        log_info("Counter WASM module shutting down");
    }
}

impl CounterModule {
    fn render(&self) {
        send_update(
            "counter_chip",
            &json!({
                "text": format!("Count: {}", self.count),
                "count": self.count
            }),
        );
    }
}

export_wasm_module!(CounterModule);
