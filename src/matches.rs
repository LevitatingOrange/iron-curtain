use crate::config::Config;
use either::{Either, Left, Right};
use eyre::{ensure, eyre, Result, WrapErr};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html};
use serde::Serialize;
use time::format_description::{self, FormatItem};
use time::{Date, Duration, OffsetDateTime, PrimitiveDateTime};
use tracing::info;

mod selector {
    use lazy_static::lazy_static;
    use scraper::Selector;

    lazy_static! {
        pub static ref MODULE: Selector = Selector::parse("div.module-gameplan").unwrap();
        pub static ref MATCH_OR_HEAD: Selector = Selector::parse("div.match,div.hs-head").unwrap();
        pub static ref MATCH: Selector = Selector::parse("div.match").unwrap();
        pub static ref HEAD_DATE: Selector = Selector::parse("div.match-date").unwrap();
        pub static ref TEAM_NAME_HOME: Selector =
            Selector::parse("div.team-name.team-name-home").unwrap();
        pub static ref TEAM_NAME_AWAY: Selector =
            Selector::parse("div.team-name.team-name-away").unwrap();
    }
}

lazy_static! {
    pub static ref DATETIME_FORMAT: Vec<FormatItem<'static>> =
        format_description::parse("[day].[month].[year] [hour]:[minute]").unwrap();
    pub static ref DATE_FORMAT: Vec<FormatItem<'static>> =
        format_description::parse("[day].[month].[year]").unwrap();
}

fn aggregate_text(element: ElementRef) -> String {
    element.text().collect()
}

fn parse_time(config: &Config, text: &str) -> Result<Either<OffsetDateTime, Date>> {
    PrimitiveDateTime::parse(text, &DATETIME_FORMAT)
        .map(|s| Left(s.assume_offset(config.utc_offset)))
        .or_else(|_| Date::parse(text, &DATE_FORMAT).map(Right))
        .wrap_err("could not parse time as either a date with or without time")
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Match {
    pub home_team: String,
    pub away_team: String,
    pub time: Either<OffsetDateTime, Date>,
}

impl Match {
    pub fn new(home_team: String, away_team: String, time: Either<OffsetDateTime, Date>) -> Self {
        Self {
            home_team,
            away_team,
            time,
        }
    }
}

pub fn get_games(config: &Config, body: &str) -> Result<Vec<Match>> {
    let html = Html::parse_document(body);

    let mut current_date = None;

    let mut matches = Vec::new();

    for e in html
        .select(&selector::MODULE)
        .next()
        .ok_or_else(|| eyre!("could not find gameplan container"))?
        .select(&selector::MATCH_OR_HEAD)
    {
        if !selector::MATCH.matches(&e) {
            current_date = e
                .select(&selector::HEAD_DATE)
                .next()
                .map(|e| {
                    let text = aggregate_text(e);
                    parse_time(&config, &text).wrap_err_with(|| {
                        format!("could not parse datetime from header '{}'", text)
                    })
                })
                .transpose()?;
        } else {
            let current_date = current_date
                .as_ref()
                .ok_or_else(|| eyre!("no date for current match"))?;

            let home_team: String = aggregate_text(
                e.select(&selector::TEAM_NAME_HOME)
                    .next()
                    .ok_or_else(|| eyre!("could not get home team name from match"))?,
            );

            ensure!(
                home_team.len() > 0,
                "could not get home team name from match"
            );

            let away_team = aggregate_text(
                e.select(&selector::TEAM_NAME_AWAY)
                    .next()
                    .ok_or_else(|| eyre!("could not get away team name from match"))?,
            );
            ensure!(
                away_team.len() > 0,
                "could not get away team name from match"
            );

            matches.push(Match::new(home_team, away_team, current_date.clone()));
        }
    }

    Ok(matches)
}
