// SPDX-License-Identifier: AGPL-3.0-or-later
use std::process::ExitCode;

use cupful::{AppState, Config};

const USAGE: &str = "usage: cupful [serve | reset-password <username> | bind-tailscale <username> <login> | import-foods <fdc.json> | seed-demo [--days N] [--seed N] [--force] | eval --provider <id>]";

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
        Some("bind-tailscale") if args.len() == 3 => bind_tailscale(&args[1], &args[2]).await,
        Some("import-foods") if args.len() == 2 => import_foods(&args[1]).await,
        Some("seed-demo") => seed_demo(&args[1..]).await,
        Some("eval") if args.len() == 3 && args[1] == "--provider" => match args[2].parse() {
            Ok(id) => eval(id).await,
            Err(_) => Err(USAGE.to_string()),
        },
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

/// Imports a FoodData Central Foundation or SR Legacy JSON file (unzipped).
async fn import_foods(path: &str) -> Result<(), String> {
    let db = open_db(&Config::from_env()?).await?;
    let (imported, skipped) =
        cupful::nutrition::foods::import_usda(&db, std::path::Path::new(path)).await?;
    eprintln!("Imported {imported} USDA foods ({skipped} skipped: missing or implausible values).");
    Ok(())
}

/// Maps an existing user to their tailnet login so Serve can sign them in.
async fn bind_tailscale(username: &str, login: &str) -> Result<(), String> {
    let db = open_db(&Config::from_env()?).await?;
    cupful::auth::tailscale::bind_tailscale(&db, username, login).await?;
    eprintln!(
        "Bound {username} to Tailscale login {}.",
        login.trim().to_lowercase()
    );
    Ok(())
}

/// `seed-demo [--days N] [--seed N] [--force]`: see `cupful::demo`.
async fn seed_demo(args: &[String]) -> Result<(), String> {
    let (mut days, mut seed, mut force) = (90u32, 42u64, false);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| USAGE.to_string())
        };
        match arg.as_str() {
            "--days" => days = u32::try_from(value()?).map_err(|_| USAGE.to_string())?,
            "--seed" => seed = value()?,
            "--force" => force = true,
            _ => return Err(USAGE.to_string()),
        }
    }
    let config = Config::from_env()?;
    let db = open_db(&config).await?;
    // The demo user's timezone is UTC.
    let today = jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .date();
    let password = cupful::demo::seed(&db, today, days, seed, force).await?;
    println!("Created user \"demo\" with {days} days of data.\nPassword (shown once): {password}");
    Ok(())
}

/// Runs the golden set against a live provider and prints per-item accuracy. Not part of CI.
async fn eval(provider_id: i64) -> Result<(), String> {
    use cupful::nutrition::golden;
    let config = Config::from_env()?;
    let state = AppState::new(open_db(&config).await?, config);
    let provider = cupful::ai::by_id(&state, provider_id)
        .await
        .map_err(|e| match e {
            cupful::ai::ProviderError::NotConfigured => format!("no such provider: {provider_id}"),
            e => format!("provider {provider_id}: {e:?}"),
        })?;
    let (mut right, mut total) = (0, 0);
    for (name, case) in golden::load(std::path::Path::new(golden::DIR))? {
        // User 0: shared foods only.
        match cupful::nutrition::estimate(
            &state.db,
            0,
            provider.as_ref(),
            &case.description,
            vec![],
        )
        .await
        {
            Ok(est) => {
                let (r, t, failures) = golden::check(&case, &est);
                println!("{name}: {r}/{t}");
                for f in failures {
                    println!("  {f}");
                }
                (right, total) = (right + r, total + t);
            }
            Err(e) => {
                println!("{name}: error {e:?}");
                total += case.expected.len();
            }
        }
    }
    println!("per-item accuracy: {right}/{total}");
    Ok(())
}

async fn serve() -> Result<(), String> {
    let config = Config::from_env()?;
    cupful::config::validate_identity(&config)?;
    let db = open_db(&config).await?;
    cupful::db::warn_if_network_filesystem(&config.data_dir);
    let env_key = std::env::var("CUPFUL_SECRET_KEY")
        .ok()
        .filter(|v| !v.is_empty());
    let secrets = cupful::crypto::load(&db, &config.data_dir, env_key.as_deref()).await?;
    let mut state = AppState::new(db.clone(), config.clone());
    state.secrets = std::sync::Arc::new(secrets);
    cupful::ai::settings::seed_from_env(&state, |k| std::env::var(k).ok()).await?;
    cupful::cleanup::spawn(db);
    cupful::google::spawn(state.clone());
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    tracing::info!("listening on http://{}", config.bind);
    let app = cupful::router(state);
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
