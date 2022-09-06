use crate::config::Config;
use crate::matches::Match;
use crate::matches::DATETIME_FORMAT;
use crate::matches::DATE_FORMAT;
use clap::{Parser, Subcommand};
use either::{Either, Left, Right};
use eyre::{ensure, eyre, Result, WrapErr};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{Client, RequestBuilder};
use scraper::{ElementRef, Html};
use serde::Serialize;
use tera::Context;
use tera::Tera;
use time::format_description::{self, FormatItem};
use time::{Date, Duration, OffsetDateTime, PrimitiveDateTime};
use tracing::info;
use url::Url;

#[derive(Debug, Serialize)]
struct PushOverMessage<'a> {
    token: &'a str,
    user: &'a str,
    title: &'a str,
    message: &'a str,
    html: u8,
    priority: i8,
    sound: &'a str,
}

impl<'a> PushOverMessage<'a> {
    fn new(
        token: &'a str,
        user: &'a str,
        title: &'a str,
        message: &'a str,
        priority: i8,
        sound: &'a str,
    ) -> Self {
        Self {
            token,
            user,
            title,
            message,
            html: 1,
            priority,
            sound,
        }
    }
}

pub async fn send_matches(
    client: &Client,
    tera: &Tera,
    config: &Config,
    regex: &Regex,
    matches: Vec<Match>,
) -> Result<()> {
    let url = Url::parse("https://api.pushover.net/1/messages.json")?;
    let token = &config.pushover.token.extract("PUSHOVER_API_TOKEN").await?;
    let user = &config
        .pushover
        .user_key
        .extract("PUSHOVER_API_USER_KEY")
        .await?;

    for the_match in matches {
        info!(
            "Sending push notification for a game on {} to {}",
            the_match.time, url
        );
        let is_home_match = regex.is_match(&the_match.home_team);
        let mut context = Context::new();
        context.insert("is_home_match", &is_home_match);
        context.insert("match", &the_match);

        let formatted_date = match the_match.time {
            Left(time) => time.format(&DATETIME_FORMAT)?,
            Right(date) => date.format(&DATE_FORMAT)?,
        };

        context.insert("formatted_date", &formatted_date);

        let message_text = tera.render("message.html", &context)?;

        let message = PushOverMessage::new(
            &token,
            &user,
            &config.pushover.notification_title,
            &message_text,
            -1,
            &config.pushover.notification_sound,
        );

        let res = client.post(url.clone()).json(&message).send().await?;
        ensure!(
            res.status() == 200,
            "server did not respond with 200: {}",
            res.text().await?
        );
    }

    Ok(())
}
