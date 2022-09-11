use clap::{Parser, Subcommand};
use config::Config;
use eyre::{Result, WrapErr};
use matches::{get_games, Match};
use pushover::send_matches;
use regex::Regex;
use reqwest::Client;
use tera::Tera;
use time::{Duration, OffsetDateTime};
use tracing::info;

mod config;
mod matches;
mod pushover;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// This programs config file location
    #[clap(short, long, default_value_t = String::from("config.toml"))]
    config_file: String,

    #[clap(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Run the server
    Run,
}

async fn run(config: Config) -> Result<()> {
    let regex = Regex::new(&config.team_regex)
        .wrap_err_with(|| format!("could not create regex from '{}'", config.team_regex))?;

    let mut tera = Tera::default();
    tera.add_raw_template("message.html", &config.pushover.notification_message)?;

    let client = Client::new();

    let mut matches = Vec::new();
    for url in &config.scrape_urls {
        info!("Checking matches from {}", url);
        let body = client
            .get(url.clone())
            .send()
            .await?
            .text()
            .await
            .wrap_err_with(|| format!("could not get html from {}", url))?;
        let mut new_matches = get_games(&config, &body)
            .wrap_err_with(|| format!("could not get games from html loaded from {}", url))?;

        matches.append(&mut new_matches);
    }

    let now = OffsetDateTime::now_utc().date();
    let search_duration = Duration::days(config.search_duration_in_days as i64);
    let mut next_applicable_matches: Vec<Match> = matches
        .into_iter()
        .filter(|m| {
            let d = m.time.right_or_else(OffsetDateTime::date) - now;
            let valid_time = d.is_zero() || (d.is_positive() && d <= search_duration);
            let valid_team = regex.is_match(&m.home_team) || regex.is_match(&m.away_team);
            valid_time && valid_team
        })
        .collect();

    next_applicable_matches.sort_by(|a, b| {
        a.time
            .right_or_else(OffsetDateTime::date)
            .cmp(&b.time.right_or_else(OffsetDateTime::date))
    });
    if next_applicable_matches.is_empty() {
        info!("No matches from sources matched configured filters");
        return Ok(());
    }

    info!(
        "{} matches from sources matched configured filters, sending notfications!",
        next_applicable_matches.len()
    );

    send_matches(&client, &tera, &config, &regex, next_applicable_matches)
        .await
        .wrap_err("could not send out matches via pushover")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let cli = Cli::parse();
    let config = Config::load(&cli.config_file).await?;
    match cli.command {
        Commands::Run => run(config).await?,
    }
    Ok(())
}
