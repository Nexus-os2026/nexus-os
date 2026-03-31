//! Screen-poster agent for governed vision-driven social posting with human approvals.

pub mod approval;
pub mod comments;
pub mod composer;
pub mod engagement;
pub mod navigator;
pub mod poster;
pub mod stealth;

#[cfg(test)]
mod tests {
    use super::*;

    // ── Stealth: seeded gaussian delays ──

    #[test]
    fn gaussian_delays_correct_count_and_bounded() {
        let profile = stealth::StealthProfile::default();
        let delays = stealth::gaussian_action_delays_ms_seeded(10, &profile, 42);
        assert_eq!(delays.len(), 10);
        for d in &delays {
            assert!(*d >= profile.min_action_delay_ms);
        }
    }

    #[test]
    fn gaussian_delays_seeded_deterministic() {
        let profile = stealth::StealthProfile::default();
        let a = stealth::gaussian_action_delays_ms_seeded(5, &profile, 99);
        let b = stealth::gaussian_action_delays_ms_seeded(5, &profile, 99);
        assert_eq!(a, b);
    }

    #[test]
    fn gaussian_delays_different_seeds_differ() {
        let profile = stealth::StealthProfile::default();
        let a = stealth::gaussian_action_delays_ms_seeded(20, &profile, 1);
        let b = stealth::gaussian_action_delays_ms_seeded(20, &profile, 2);
        assert_ne!(a, b);
    }

    // ── Stealth: typing delays ──

    #[test]
    fn typing_delays_match_char_count() {
        let profile = stealth::StealthProfile::default();
        let text = "hello world";
        let delays = stealth::typing_delays_ms_seeded(text, &profile, 42);
        assert_eq!(delays.len(), text.chars().count());
    }

    #[test]
    fn typing_delays_bounded() {
        let profile = stealth::StealthProfile::default();
        let delays = stealth::typing_delays_ms_seeded("type some text here", &profile, 42);
        for d in &delays {
            assert!(*d >= profile.min_typing_delay_ms);
            assert!(*d <= profile.max_typing_delay_ms);
        }
    }

    #[test]
    fn typing_delays_empty_text() {
        let profile = stealth::StealthProfile::default();
        let delays = stealth::typing_delays_ms_seeded("", &profile, 42);
        assert!(delays.is_empty());
    }

    // ── Stealth: bezier mouse path ──

    #[test]
    fn bezier_path_starts_and_ends_at_endpoints() {
        let start = (0.0, 0.0);
        let end = (100.0, 200.0);
        let ctrl1 = (30.0, 80.0);
        let ctrl2 = (70.0, 150.0);
        let path = stealth::bezier_mouse_path(start, end, ctrl1, ctrl2, 50);
        assert_eq!(path.len(), 50);
        let (fx, fy) = path.first().unwrap();
        let (lx, ly) = path.last().unwrap();
        assert!((fx - start.0).abs() < 1e-6);
        assert!((fy - start.1).abs() < 1e-6);
        assert!((lx - end.0).abs() < 1e-6);
        assert!((ly - end.1).abs() < 1e-6);
    }

    #[test]
    fn bezier_path_minimum_two_points() {
        let path = stealth::bezier_mouse_path((0.0, 0.0), (1.0, 1.0), (0.5, 0.5), (0.5, 0.5), 1);
        assert!(path.len() >= 2);
    }

    // ── Session guard ──

    #[test]
    fn session_guard_allows_within_duration() {
        let profile = stealth::StealthProfile {
            max_session_duration_secs: 3600,
            ..stealth::StealthProfile::default()
        };
        let guard = stealth::SessionGuard::new(profile);
        assert!(guard.allow_session(3599));
        assert!(guard.allow_session(3600));
        assert!(!guard.allow_session(3601));
    }

    #[test]
    fn session_guard_rate_limits_posts() {
        let profile = stealth::StealthProfile {
            max_posts_per_hour: 3,
            ..stealth::StealthProfile::default()
        };
        let mut guard = stealth::SessionGuard::new(profile);
        let now = 10000;
        assert!(guard.allow_post(now));
        assert!(guard.allow_post(now + 1));
        assert!(guard.allow_post(now + 2));
        assert!(!guard.allow_post(now + 3)); // 4th post in same hour window
    }

    #[test]
    fn session_guard_expires_old_posts() {
        let profile = stealth::StealthProfile {
            max_posts_per_hour: 2,
            ..stealth::StealthProfile::default()
        };
        let mut guard = stealth::SessionGuard::new(profile);
        assert!(guard.allow_post(1000));
        assert!(guard.allow_post(1001));
        assert!(!guard.allow_post(1002)); // 3rd blocked
                                          // Fast forward past 1 hour
        assert!(guard.allow_post(4602)); // old posts expired
    }

    // ── Navigator: platform labels ──

    #[test]
    fn platform_labels_all_lowercase() {
        use navigator::SocialPlatform;
        let platforms = [
            SocialPlatform::X,
            SocialPlatform::Instagram,
            SocialPlatform::Facebook,
            SocialPlatform::Reddit,
            SocialPlatform::LinkedIn,
            SocialPlatform::TikTok,
            SocialPlatform::YouTube,
        ];
        for p in &platforms {
            let label = p.as_label();
            assert_eq!(
                label,
                label.to_ascii_lowercase(),
                "platform {p:?} label not lowercase"
            );
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn platform_navigation_maps_have_urls() {
        use navigator::SocialPlatform;
        let platforms = [
            SocialPlatform::X,
            SocialPlatform::Instagram,
            SocialPlatform::Facebook,
            SocialPlatform::Reddit,
            SocialPlatform::LinkedIn,
            SocialPlatform::TikTok,
            SocialPlatform::YouTube,
        ];
        for p in &platforms {
            let map = p.navigation_map();
            assert!(
                map.url.starts_with("https://"),
                "platform {p:?} URL missing https"
            );
            assert!(
                !map.logged_in_selectors.is_empty(),
                "platform {p:?} no login selectors"
            );
            assert!(
                !map.new_post_selectors.is_empty(),
                "platform {p:?} no post selectors"
            );
        }
    }

    // ── Stealth profile serialization ──

    #[test]
    fn stealth_profile_roundtrip() {
        let profile = stealth::StealthProfile::default();
        let json = serde_json::to_string(&profile).unwrap();
        let restored: stealth::StealthProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, profile);
    }
}
