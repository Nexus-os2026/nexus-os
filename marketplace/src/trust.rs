use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CapabilityRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub user_id: String,
    pub rating: u8,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorReputation {
    pub author_id: String,
    pub score: f32,
    pub review_count: usize,
}

#[derive(Debug, Default)]
pub struct TrustSystem {
    package_author: HashMap<String, String>,
    package_reviews: HashMap<String, Vec<Review>>,
}

impl TrustSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_package(&mut self, package_id: &str, author_id: &str) {
        self.package_author
            .insert(package_id.to_string(), author_id.to_string());
    }

    pub fn add_review(&mut self, package_id: &str, review: Review) {
        let rating = review.rating.clamp(1, 5);
        let sanitized = Review {
            rating,
            ..review
        };
        self.package_reviews
            .entry(package_id.to_string())
            .or_default()
            .push(sanitized);
    }

    pub fn author_reputation(&self, author_id: &str) -> AuthorReputation {
        let ratings = self
            .package_author
            .iter()
            .filter(|(_, author)| author.as_str() == author_id)
            .flat_map(|(package_id, _)| {
                self.package_reviews
                    .get(package_id.as_str())
                    .into_iter()
                    .flatten()
            })
            .map(|review| review.rating as f32)
            .collect::<Vec<_>>();

        let review_count = ratings.len();
        let score = if review_count == 0 {
            50.0
        } else {
            let avg = ratings.iter().copied().sum::<f32>() / review_count as f32;
            (avg / 5.0) * 100.0
        };

        AuthorReputation {
            author_id: author_id.to_string(),
            score,
            review_count,
        }
    }

    pub fn classify_capability(capability: &str) -> CapabilityRisk {
        match capability {
            "llm.query" | "web.search" | "audit.read" => CapabilityRisk::Low,
            "social.post" | "messaging.send" | "fs.read" => CapabilityRisk::Medium,
            "fs.write" | "screen.capture" | "input.keyboard" | "shell.exec" => {
                CapabilityRisk::High
            }
            _ => CapabilityRisk::Medium,
        }
    }

    pub fn classify_capability_set(capabilities: &[String]) -> CapabilityRisk {
        capabilities
            .iter()
            .map(|capability| Self::classify_capability(capability.as_str()))
            .max()
            .unwrap_or(CapabilityRisk::Low)
    }
}

#[cfg(test)]
mod tests {
    use super::{CapabilityRisk, Review, TrustSystem};

    #[test]
    fn test_author_reputation() {
        let mut trust = TrustSystem::new();
        trust.register_package("pkg-a", "author-1");
        trust.add_review(
            "pkg-a",
            Review {
                user_id: "u1".to_string(),
                rating: 5,
                comment: "solid".to_string(),
            },
        );
        trust.add_review(
            "pkg-a",
            Review {
                user_id: "u2".to_string(),
                rating: 4,
                comment: "good".to_string(),
            },
        );

        let rep = trust.author_reputation("author-1");
        assert_eq!(rep.review_count, 2);
        assert!(rep.score >= 80.0);
    }

    #[test]
    fn test_capability_risk() {
        let risk = TrustSystem::classify_capability_set(&[
            "llm.query".to_string(),
            "screen.capture".to_string(),
        ]);
        assert_eq!(risk, CapabilityRisk::High);
    }
}
