use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::handler;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let api_v1 = Router::new()
        .route("/transactions", get(handler::list_transactions))
        .route("/transactions/{signature}", get(handler::get_transaction))
        .route(
            "/accounts/{address}/transactions",
            get(handler::get_account_transactions),
        );

    Router::new()
        .nest("/api/v1", api_v1)
        .route("/health", get(handler::health_check))
        .route("/ws/transactions", get(handler::ws_transactions))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
