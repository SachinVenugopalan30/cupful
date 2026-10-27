// SPDX-License-Identifier: AGPL-3.0-or-later
use std::process::ExitCode;

use cupful::{AppState, Config};

const USAGE: &str = "usage: cupful [serve | reset-password <username>]";

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
        Some("reset-password") if args.len() == 2 => reset_password(&args[1]).await,
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

async fn open_db(config: &Config) -> Result<sqlx::SqlitePool, String> {
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|e| format!("create {}: {e}", config.data_dir.display()))?;
    cupful::db::open(&config.data_dir.join("cupful.db"))
        .await
        .map_err(|e| format!("open database: {e}"))
}

/// Reads the new password from stdin (visible if typed; pipe it to avoid that).
async fn reset_password(username: &str) -> Result<(), String> {
    let db = open_db(&Config::from_env()?).await?;
    eprint!("New password for {username}: ");
    let mut password = String::new();
    std::io::stdin()
        .read_line(&mut password)
        .map_err(|e| e.to_string())?;
    cupful::auth::reset_password(&db, username, password.trim_end_matches(['\r', '\n'])).await?;
    eprintln!("Password reset; {username}'s sessions were ended.");
    Ok(())
}

async fn serve() -> Result<(), String> {
    let config = Config::from_env()?;
    let db = open_db(&config).await?;
    cupful::cleanup::spawn(db.clone());
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    tracing::info!("listening on http://{}", config.bind);
    let app = cupful::router(AppState::new(db, config));
    // ConnectInfo gives the login rate limiter the peer address.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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
