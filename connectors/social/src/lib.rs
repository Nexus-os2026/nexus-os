//! Social connectors for publishing governed content to X, Facebook, and Instagram.

pub mod facebook;
pub mod instagram;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facebook_connector_creates_with_zero_posts() {
        let connector = facebook::FacebookConnector::new();
        assert_eq!(connector.published_count(), 0);
    }

    #[test]
    fn facebook_page_metrics_default_is_zero() {
        let connector = facebook::FacebookConnector::new();
        let metrics = connector.page_metrics();
        assert_eq!(metrics.impressions, 0);
        assert_eq!(metrics.engagement, 0);
    }

    #[test]
    fn instagram_connector_creates_with_zero_posts() {
        let connector = instagram::InstagramConnector::new();
        assert_eq!(connector.published_count(), 0);
    }
}
