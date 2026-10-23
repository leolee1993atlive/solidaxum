use std::{sync::LazyLock, time::Instant};

use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusHandle;

static HANDLE: LazyLock<PrometheusHandle> = LazyLock::new(|| {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let recorder = builder.build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder)
        .expect("Failed to set metrics recorder");
    handle
});

pub fn init_metrics() {
    LazyLock::force(&HANDLE);
    tracing::info!("Prometheus metrics recorder 已安装");
}

pub fn metrics_output() -> String {
    HANDLE.render()
}

pub struct RequestTimer {
    method: String,
    path: String,
    start: Instant,
}

impl RequestTimer {
    pub fn new(method: String, path: String) -> Self {
        Self {
            method,
            path,
            start: Instant::now(),
        }
    }
}

impl Drop for RequestTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        histogram!(
            "http_request_duration_seconds",
            "method" => self.method.clone(),
            "path" => self.path.clone(),
        )
        .record(elapsed);
    }
}

pub fn record_http_request(method: &str, path: &str, status: u16) {
    counter!(
        "http_requests_total",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string(),
    )
    .increment(1);
}
