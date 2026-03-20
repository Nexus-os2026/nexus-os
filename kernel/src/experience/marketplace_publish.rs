//! Marketplace Publishing — share or sell project templates.
//!
//! Users can publish successful projects as reusable templates that others
//! can install with one click and customise via the Remix engine.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pricing model for a published template.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Pricing {
    Free,
    OneTime(u64),
    Subscription { monthly_cents: u64 },
}

/// A published marketplace listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceListing {
    pub listing_id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub pricing: Pricing,
    pub screenshots: Vec<String>,
    pub install_count: u64,
    pub rating: f64,
    pub published_at: u64,
}

/// Engine that handles packaging and publishing projects to the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePublisher {
    pub listings: Vec<MarketplaceListing>,
}

impl Default for MarketplacePublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketplacePublisher {
    pub fn new() -> Self {
        Self {
            listings: Vec::new(),
        }
    }

    /// Publish a project as a marketplace template.
    pub fn publish(
        &mut self,
        project_id: &str,
        title: &str,
        description: &str,
        pricing: Pricing,
        screenshots: Vec<String>,
    ) -> MarketplaceListing {
        let listing = MarketplaceListing {
            listing_id: Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            pricing,
            screenshots,
            install_count: 0,
            rating: 0.0,
            published_at: now_secs(),
        };
        self.listings.push(listing.clone());
        listing
    }

    /// Install a template by listing ID.  Returns the listing if found.
    pub fn install(&mut self, listing_id: &str) -> Option<MarketplaceListing> {
        if let Some(listing) = self
            .listings
            .iter_mut()
            .find(|l| l.listing_id == listing_id)
        {
            listing.install_count += 1;
            Some(listing.clone())
        } else {
            None
        }
    }

    /// List all published templates.
    pub fn list(&self) -> &[MarketplaceListing] {
        &self.listings
    }

    /// Search listings by keyword.
    pub fn search(&self, query: &str) -> Vec<&MarketplaceListing> {
        let q = query.to_lowercase();
        self.listings
            .iter()
            .filter(|l| {
                l.title.to_lowercase().contains(&q) || l.description.to_lowercase().contains(&q)
            })
            .collect()
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_list() {
        let mut publisher = MarketplacePublisher::new();
        let listing = publisher.publish(
            "proj-1",
            "T-Shirt Store",
            "Complete e-commerce template",
            Pricing::Free,
            vec![],
        );
        assert_eq!(listing.title, "T-Shirt Store");
        assert_eq!(publisher.list().len(), 1);
    }

    #[test]
    fn test_install_increments_count() {
        let mut publisher = MarketplacePublisher::new();
        let listing = publisher.publish("proj-1", "Store", "desc", Pricing::Free, vec![]);
        let id = listing.listing_id.clone();

        let installed = publisher.install(&id).unwrap();
        assert_eq!(installed.install_count, 1);
        let installed2 = publisher.install(&id).unwrap();
        assert_eq!(installed2.install_count, 2);
    }

    #[test]
    fn test_install_not_found() {
        let mut publisher = MarketplacePublisher::new();
        assert!(publisher.install("nonexistent").is_none());
    }

    #[test]
    fn test_search() {
        let mut publisher = MarketplacePublisher::new();
        publisher.publish("p1", "T-Shirt Store", "e-commerce", Pricing::Free, vec![]);
        publisher.publish("p2", "Portfolio", "personal site", Pricing::Free, vec![]);
        let results = publisher.search("store");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "T-Shirt Store");
    }

    #[test]
    fn test_pricing_variants() {
        let mut publisher = MarketplacePublisher::new();
        let l1 = publisher.publish("p1", "Free", "d", Pricing::Free, vec![]);
        let l2 = publisher.publish("p2", "Paid", "d", Pricing::OneTime(2900), vec![]);
        let l3 = publisher.publish(
            "p3",
            "Sub",
            "d",
            Pricing::Subscription { monthly_cents: 999 },
            vec![],
        );
        assert_eq!(l1.pricing, Pricing::Free);
        assert_eq!(l2.pricing, Pricing::OneTime(2900));
        assert_eq!(l3.pricing, Pricing::Subscription { monthly_cents: 999 });
    }
}
