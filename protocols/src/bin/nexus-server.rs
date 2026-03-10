//! Nexus OS server binary — starts the HTTP gateway with full kernel governance.
//!
//! Listens for SIGTERM / Ctrl-C to trigger graceful shutdown:
//! stop agents, flush audit, then exit within 5 seconds.

use nexus_protocols::http_gateway::{build_router, GatewayState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "nexus-default-secret".to_string());

    let state = GatewayState::new(&jwt_secret);
    let shutdown_state = state.clone();
    let router = build_router(state);

    let http_addr = std::env::var("NEXUS_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = TcpListener::bind(&http_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {http_addr}: {e}"));

    println!("Nexus OS gateway listening on {http_addr}");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(shutdown_state))
        .await
        .expect("server error");

    println!("Nexus OS gateway exited");
}

async fn shutdown_signal(state: GatewayState) {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => println!("Received Ctrl-C, shutting down..."),
            _ = sigterm.recv() => println!("Received SIGTERM, shutting down..."),
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("failed to listen for Ctrl-C");
        println!("Received Ctrl-C, shutting down...");
    }

    // Run shutdown with a 5-second deadline
    let shutdown = tokio::task::spawn_blocking(move || {
        state.shutdown();
    });

    let deadline = tokio::time::timeout(std::time::Duration::from_secs(5), shutdown);
    match deadline.await {
        Ok(Ok(())) => println!("Graceful shutdown complete"),
        _ => {
            eprintln!("Shutdown timed out after 5 seconds, forcing exit");
            std::process::exit(1);
        }
    }
}
