// radio-browser.info client — free, no API key, community station database

use anyhow::{Context, Result};
use serde::Deserialize;

const API: &str = "https://de1.api.radio-browser.info/json";

#[derive(Debug, Clone, Deserialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bitrate: u32,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub tags: String,
}

/// Search stations by name. `limit` caps results.
pub fn search(name: &str, limit: u32) -> Result<Vec<Station>> {
    let url = format!("{API}/stations/search?name={name}&limit={limit}&hidebroken=true&order=votes&reverse=true");
    let stations: Vec<Station> = reqwest::blocking::get(&url)
        .context("radio-browser request failed")?
        .json()
        .context("radio-browser returned bad JSON")?;
    Ok(stations.into_iter().filter(|s| !s.url.is_empty()).collect())
}

/// Top-voted stations, for a "just play something good" default view.
pub fn top_voted(limit: u32) -> Result<Vec<Station>> {
    let url = format!("{API}/stations/topvote/{limit}");
    let stations: Vec<Station> = reqwest::blocking::get(&url)
        .context("radio-browser request failed")?
        .json()
        .context("radio-browser returned bad JSON")?;
    Ok(stations.into_iter().filter(|s| !s.url.is_empty()).collect())
}
