// SPDX-License-Identifier: AGPL-3.0-or-later
use std::process::ExitCode;

use cupful::{AppState, Config};

const USAGE: &str = "usage: cupful [serve]";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None | Some("serve") => serve().await,
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), String> {
    let config = Config::from_env()?;
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|e| format!("create {}: {e}", config.data_dir.display()))?;
    let db = cupful::db::open(&config.data_dir.join("cupful.db"))
        .await
        .map_err(|e| format!("open database: {e}"))?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    tracing::info!("listening on http://{}", config.bind);
    let app = cupful::router(AppState::new(db, config));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|e| e.to_string())
}

/// Ctrl-C locally, SIGTERM from `docker stop`.
async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}
