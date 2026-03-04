use prometheus::{
    Encoder, Gauge, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    Opts, Registry, TextEncoder,
};
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,

    pub trades_total: IntCounterVec,
    pub trade_latency: HistogramVec,
    pub simulation_latency: Histogram,
    pub slippage_bps: HistogramVec,
    pub open_positions: IntGauge,
    pub portfolio_exposure: Gauge,
    pub stream_reconnects: IntCounter,
    pub stream_lag_slots: IntGauge,
    pub jupiter_cache_hits: IntCounter,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let trades_total = IntCounterVec::new(
            Opts::new("copytrader_trades_total", "Total trades by outcome"),
            &["target", "status", "dex"],
        )
        .unwrap();

        let trade_latency = HistogramVec::new(
            HistogramOpts::new(
                "copytrader_trade_latency_ms",
                "End-to-end latency (detect → confirm)",
            )
            .buckets(vec![50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0]),
            &["target", "dex"],
        )
        .unwrap();

        let simulation_latency = Histogram::with_opts(
            HistogramOpts::new(
                "copytrader_simulation_latency_ms",
                "simulateTransaction round-trip time",
            )
            .buckets(vec![25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
        )
        .unwrap();

        let slippage_bps = HistogramVec::new(
            HistogramOpts::new(
                "copytrader_slippage_bps",
                "Actual slippage observed on filled trades",
            )
            .buckets(vec![10.0, 50.0, 100.0, 200.0, 500.0, 1000.0]),
            &["dex"],
        )
        .unwrap();

        let open_positions = IntGauge::with_opts(Opts::new(
            "copytrader_open_positions",
            "Current open position count",
        ))
        .unwrap();

        let portfolio_exposure = Gauge::with_opts(Opts::new(
            "copytrader_portfolio_exposure_sol",
            "Total SOL value in open positions",
        ))
        .unwrap();

        let stream_reconnects = IntCounter::with_opts(Opts::new(
            "copytrader_stream_reconnects_total",
            "gRPC stream reconnection count",
        ))
        .unwrap();

        let stream_lag_slots = IntGauge::with_opts(Opts::new(
            "copytrader_stream_lag_slots",
            "Slots behind tip",
        ))
        .unwrap();

        let jupiter_cache_hits = IntCounter::with_opts(Opts::new(
            "copytrader_jupiter_quote_cache_hits",
            "Redis price cache hit count",
        ))
        .unwrap();

        registry.register(Box::new(trades_total.clone())).unwrap();
        registry.register(Box::new(trade_latency.clone())).unwrap();
        registry
            .register(Box::new(simulation_latency.clone()))
            .unwrap();
        registry.register(Box::new(slippage_bps.clone())).unwrap();
        registry
            .register(Box::new(open_positions.clone()))
            .unwrap();
        registry
            .register(Box::new(portfolio_exposure.clone()))
            .unwrap();
        registry
            .register(Box::new(stream_reconnects.clone()))
            .unwrap();
        registry
            .register(Box::new(stream_lag_slots.clone()))
            .unwrap();
        registry
            .register(Box::new(jupiter_cache_hits.clone()))
            .unwrap();

        Self {
            registry: Arc::new(registry),
            trades_total,
            trade_latency,
            simulation_latency,
            slippage_bps,
            open_positions,
            portfolio_exposure,
            stream_reconnects,
            stream_lag_slots,
            jupiter_cache_hits,
        }
    }
}

pub async fn serve_metrics(
    listen: String,
    metrics: Arc<Metrics>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&listen).await?;
    tracing::info!(listen = %listen, "Metrics server started");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _addr) = result?;
                let io = TokioIo::new(stream);
                let metrics = metrics.clone();

                tokio::spawn(async move {
                    let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                        let metrics = metrics.clone();
                        async move {
                            handle_request(req, metrics).await
                        }
                    });

                    if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                        tracing::debug!(error = %e, "HTTP connection error");
                    }
                });
            }
            _ = shutdown_rx.changed() => {
                tracing::info!("Metrics server shutting down");
                return Ok(());
            }
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    metrics: Arc<Metrics>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    match req.uri().path() {
        "/metrics" => {
            let encoder = TextEncoder::new();
            let metric_families = metrics.registry.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();

            Ok(Response::builder()
                .status(200)
                .header("Content-Type", encoder.format_type())
                .body(Full::new(Bytes::from(buffer)))
                .unwrap())
        }
        "/health" => {
            let body = serde_json::json!({
                "status": "healthy",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap())
        }
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap()),
    }
}
