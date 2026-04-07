//! Curated registry of free, no-auth APIs that agents can discover and use.
//!
//! Every entry has been verified to work without API keys or signup. Agents
//! receive this registry in their planning prompt so they can call structured
//! APIs instead of scraping web pages.

use serde::{Deserialize, Serialize};

/// API category for grouping and discovery.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiCategory {
    News,
    Finance,
    Weather,
    Tech,
    Knowledge,
    Government,
    Utilities,
}

impl std::fmt::Display for ApiCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::News => write!(f, "News & Information"),
            Self::Finance => write!(f, "Finance & Crypto"),
            Self::Weather => write!(f, "Weather & Geo"),
            Self::Tech => write!(f, "Tech & Dev"),
            Self::Knowledge => write!(f, "General Knowledge"),
            Self::Government => write!(f, "Government & Open Data"),
            Self::Utilities => write!(f, "Utilities"),
        }
    }
}

/// A free API endpoint that requires no authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeApi {
    pub name: String,
    pub base_url: String,
    pub category: ApiCategory,
    pub rate_limit: Option<String>,
    pub description: String,
    /// A ready-to-use example URL that returns useful data.
    pub example_endpoint: String,
}

/// The built-in registry of curated free APIs.
pub fn builtin_registry() -> Vec<FreeApi> {
    vec![
        // ── News & Information ──
        FreeApi {
            name: "HackerNews".into(),
            base_url: "https://hacker-news.firebaseio.com/v0".into(),
            category: ApiCategory::News,
            rate_limit: None,
            description: "Tech news — top stories, new stories, comments, user profiles".into(),
            example_endpoint: "https://hacker-news.firebaseio.com/v0/topstories.json?print=pretty"
                .into(),
        },
        FreeApi {
            name: "HackerNews Item".into(),
            base_url: "https://hacker-news.firebaseio.com/v0/item".into(),
            category: ApiCategory::News,
            rate_limit: None,
            description: "Fetch a single HN story/comment by ID. Use after getting IDs from topstories".into(),
            example_endpoint: "https://hacker-news.firebaseio.com/v0/item/1.json?print=pretty"
                .into(),
        },
        FreeApi {
            name: "Wikipedia".into(),
            base_url: "https://en.wikipedia.org/api/rest_v1".into(),
            category: ApiCategory::News,
            rate_limit: Some("200/sec".into()),
            description: "Summaries, full articles, and search across all of Wikipedia".into(),
            example_endpoint:
                "https://en.wikipedia.org/api/rest_v1/page/summary/Rust_(programming_language)"
                    .into(),
        },
        FreeApi {
            name: "Reddit RSS".into(),
            base_url: "https://www.reddit.com".into(),
            category: ApiCategory::News,
            rate_limit: Some("60/min".into()),
            description: "Any subreddit as RSS. Replace {sub} with subreddit name".into(),
            example_endpoint: "https://www.reddit.com/r/technology/.rss?limit=10".into(),
        },
        FreeApi {
            name: "Wikidata".into(),
            base_url: "https://www.wikidata.org/w/api.php".into(),
            category: ApiCategory::Knowledge,
            rate_limit: None,
            description: "Structured knowledge base — entities, properties, relationships".into(),
            example_endpoint: "https://www.wikidata.org/w/api.php?action=wbsearchentities&search=Rust+programming&language=en&format=json".into(),
        },
        // ── Finance & Crypto ──
        FreeApi {
            name: "CoinGecko".into(),
            base_url: "https://api.coingecko.com/api/v3".into(),
            category: ApiCategory::Finance,
            rate_limit: Some("30/min".into()),
            description: "Cryptocurrency prices, market data, coin info. No auth needed".into(),
            example_endpoint: "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum&vs_currencies=usd".into(),
        },
        FreeApi {
            name: "Exchange Rates".into(),
            base_url: "https://open.er-api.com/v6/latest".into(),
            category: ApiCategory::Finance,
            rate_limit: Some("1500/month".into()),
            description: "Live currency exchange rates for 150+ currencies".into(),
            example_endpoint: "https://open.er-api.com/v6/latest/USD".into(),
        },
        // ── Weather & Geo ──
        FreeApi {
            name: "Open-Meteo".into(),
            base_url: "https://api.open-meteo.com/v1".into(),
            category: ApiCategory::Weather,
            rate_limit: Some("10000/day".into()),
            description: "Global weather forecasts, current conditions, historical data. No auth".into(),
            example_endpoint: "https://api.open-meteo.com/v1/forecast?latitude=37.77&longitude=-122.42&current_weather=true".into(),
        },
        FreeApi {
            name: "IP Geolocation".into(),
            base_url: "https://ipapi.co".into(),
            category: ApiCategory::Weather,
            rate_limit: Some("1000/day".into()),
            description: "IP address geolocation — city, country, timezone, ISP".into(),
            example_endpoint: "https://ipapi.co/json/".into(),
        },
        FreeApi {
            name: "Country Info".into(),
            base_url: "https://restcountries.com/v3.1".into(),
            category: ApiCategory::Weather,
            rate_limit: None,
            description: "Detailed info on every country — population, languages, currencies".into(),
            example_endpoint: "https://restcountries.com/v3.1/name/japan".into(),
        },
        // ── Tech & Dev ──
        FreeApi {
            name: "GitHub".into(),
            base_url: "https://api.github.com".into(),
            category: ApiCategory::Tech,
            rate_limit: Some("60/hour unauthenticated".into()),
            description: "Public repos, search, trending, user profiles. No auth for read-only".into(),
            example_endpoint: "https://api.github.com/search/repositories?q=language:rust+stars:>1000&sort=stars&per_page=10".into(),
        },
        FreeApi {
            name: "Crates.io".into(),
            base_url: "https://crates.io/api/v1".into(),
            category: ApiCategory::Tech,
            rate_limit: Some("1/sec".into()),
            description: "Rust package registry — crate info, versions, downloads".into(),
            example_endpoint: "https://crates.io/api/v1/crates?q=serde&per_page=5".into(),
        },
        FreeApi {
            name: "NPM Registry".into(),
            base_url: "https://registry.npmjs.org".into(),
            category: ApiCategory::Tech,
            rate_limit: None,
            description: "JavaScript package registry — package metadata, versions".into(),
            example_endpoint: "https://registry.npmjs.org/react/latest".into(),
        },
        // ── General Knowledge ──
        FreeApi {
            name: "Open Library".into(),
            base_url: "https://openlibrary.org".into(),
            category: ApiCategory::Knowledge,
            rate_limit: None,
            description: "Books — search, editions, authors, covers. Open data".into(),
            example_endpoint: "https://openlibrary.org/search.json?q=artificial+intelligence&limit=5".into(),
        },
        FreeApi {
            name: "NASA".into(),
            base_url: "https://api.nasa.gov".into(),
            category: ApiCategory::Knowledge,
            rate_limit: Some("1000/hour with DEMO_KEY".into()),
            description: "Astronomy picture of the day, Mars photos, near-Earth objects".into(),
            example_endpoint: "https://api.nasa.gov/planetary/apod?api_key=DEMO_KEY".into(),
        },
        FreeApi {
            name: "Archive.org".into(),
            base_url: "https://archive.org".into(),
            category: ApiCategory::Knowledge,
            rate_limit: None,
            description: "Internet Archive — books, audio, video, web archives, Wayback Machine".into(),
            example_endpoint: "https://archive.org/advancedsearch.php?q=artificial+intelligence&output=json&rows=5".into(),
        },
        // ── Government & Open Data ──
        FreeApi {
            name: "Data.gov".into(),
            base_url: "https://catalog.data.gov/api/3".into(),
            category: ApiCategory::Government,
            rate_limit: None,
            description: "US government open data catalog — datasets, organizations".into(),
            example_endpoint: "https://catalog.data.gov/api/3/action/package_search?q=climate&rows=5".into(),
        },
        // ── Utilities ──
        FreeApi {
            name: "HTTPBin".into(),
            base_url: "https://httpbin.org".into(),
            category: ApiCategory::Utilities,
            rate_limit: None,
            description: "HTTP testing — echo requests, test headers, status codes".into(),
            example_endpoint: "https://httpbin.org/ip".into(),
        },
    ]
}

/// Check if a URL matches a known free API in the registry.
pub fn is_known_free_api(url: &str) -> bool {
    builtin_registry()
        .iter()
        .any(|api| url.starts_with(&api.base_url))
}

/// Format the registry as a compact prompt section for the cognitive planner.
pub fn registry_prompt_section() -> String {
    let mut lines = vec![
        "FREE DATA APIS (use ApiCall or ShellCommand with curl — no auth required):".to_string(),
    ];

    let registry = builtin_registry();
    let mut current_category = String::new();

    for api in &registry {
        let cat = api.category.to_string();
        if cat != current_category {
            lines.push(format!("\n  [{cat}]"));
            current_category = cat;
        }
        lines.push(format!(
            "  - {}: {} (e.g. {})",
            api.name, api.description, api.example_endpoint
        ));
    }

    lines.push("\nUse these structured APIs instead of web search when the data is available. They return JSON — faster and more reliable than scraping HTML.".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_entries() {
        let reg = builtin_registry();
        assert!(
            reg.len() >= 15,
            "expected at least 15 APIs, got {}",
            reg.len()
        );
    }

    #[test]
    fn all_entries_have_example_endpoints() {
        for api in builtin_registry() {
            assert!(
                api.example_endpoint.starts_with("http"),
                "API '{}' has invalid example: {}",
                api.name,
                api.example_endpoint
            );
        }
    }

    #[test]
    fn known_api_detection() {
        assert!(is_known_free_api(
            "https://api.coingecko.com/api/v3/simple/price"
        ));
        assert!(is_known_free_api(
            "https://hacker-news.firebaseio.com/v0/topstories.json"
        ));
        assert!(!is_known_free_api("https://evil.example.com/api"));
    }

    #[test]
    fn prompt_section_is_non_empty() {
        let section = registry_prompt_section();
        assert!(section.contains("HackerNews"));
        assert!(section.contains("CoinGecko"));
        assert!(section.contains("Open-Meteo"));
        assert!(section.len() > 500);
    }
}
