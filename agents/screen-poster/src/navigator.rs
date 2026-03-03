use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};

pub const PLAYWRIGHT_VERSION: &str = "0.12";
pub const VISION_FALLBACK_VERSION: &str = "0.12.5";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocialPlatform {
    X,
    Instagram,
    Facebook,
    Reddit,
    LinkedIn,
    TikTok,
    YouTube,
}

impl SocialPlatform {
    pub fn as_label(&self) -> &'static str {
        match self {
            SocialPlatform::X => "x",
            SocialPlatform::Instagram => "instagram",
            SocialPlatform::Facebook => "facebook",
            SocialPlatform::Reddit => "reddit",
            SocialPlatform::LinkedIn => "linkedin",
            SocialPlatform::TikTok => "tiktok",
            SocialPlatform::YouTube => "youtube",
        }
    }

    pub fn navigation_map(&self) -> PlatformNavigationMap {
        match self {
            SocialPlatform::X => PlatformNavigationMap {
                url: "https://x.com/home",
                logged_in_selectors: &[
                    "[data-testid='SideNav_AccountSwitcher_Button']",
                    "a[href='/compose/post']",
                ],
                new_post_selectors: &[
                    "a[href='/compose/post']",
                    "[data-testid='SideNav_NewTweet_Button']",
                ],
                comment_section_selectors: &["[data-testid='reply']", "article [role='group']"],
                like_button_selectors: &["[data-testid='like']", "button[aria-label*='Like']"],
                publish_button_selectors: &[
                    "[data-testid='tweetButtonInline']",
                    "button[data-testid='tweetButton']",
                ],
            },
            SocialPlatform::Instagram => PlatformNavigationMap {
                url: "https://www.instagram.com/",
                logged_in_selectors: &["svg[aria-label='New post']", "a[href='/accounts/edit/']"],
                new_post_selectors: &["svg[aria-label='New post']", "[role='menuitem']"],
                comment_section_selectors: &[
                    "textarea[aria-label='Add a comment…']",
                    "ul[role='list']",
                ],
                like_button_selectors: &["svg[aria-label='Like']", "button[aria-label='Like']"],
                publish_button_selectors: &["div[role='button']", "button[type='submit']"],
            },
            SocialPlatform::Facebook => PlatformNavigationMap {
                url: "https://www.facebook.com/",
                logged_in_selectors: &["[aria-label='Account']", "[aria-label='Create a post']"],
                new_post_selectors: &["[aria-label='Create a post']", "div[role='textbox']"],
                comment_section_selectors: &[
                    "div[aria-label='Leave a comment']",
                    "form[role='presentation']",
                ],
                like_button_selectors: &[
                    "div[aria-label='Like']",
                    "div[role='button'][aria-label*='Like']",
                ],
                publish_button_selectors: &["div[aria-label='Post']", "button[type='submit']"],
            },
            SocialPlatform::Reddit => PlatformNavigationMap {
                url: "https://www.reddit.com/",
                logged_in_selectors: &["a[href*='submit']", "shreddit-post"],
                new_post_selectors: &["a[href*='submit']", "button[aria-label='Create Post']"],
                comment_section_selectors: &["shreddit-comment", "textarea[name='comment']"],
                like_button_selectors: &["button[aria-label*='upvote']", "faceplate-partial"],
                publish_button_selectors: &["button[type='submit']", "button[aria-label='Post']"],
            },
            SocialPlatform::LinkedIn => PlatformNavigationMap {
                url: "https://www.linkedin.com/feed/",
                logged_in_selectors: &[
                    "button[aria-label='Start a post']",
                    "a[href='/mynetwork/']",
                ],
                new_post_selectors: &["button[aria-label='Start a post']", "div[role='textbox']"],
                comment_section_selectors: &[
                    "div.comments-comment-box__form-container",
                    "textarea[name='comment']",
                ],
                like_button_selectors: &[
                    "button[aria-label*='Like']",
                    "button.react-button__trigger",
                ],
                publish_button_selectors: &[
                    "button.share-actions__primary-action",
                    "button[aria-label='Post']",
                ],
            },
            SocialPlatform::TikTok => PlatformNavigationMap {
                url: "https://www.tiktok.com/",
                logged_in_selectors: &["a[href*='upload']", "button[data-e2e='profile-icon']"],
                new_post_selectors: &["a[href*='upload']", "button[data-e2e='upload-icon']"],
                comment_section_selectors: &[
                    "div[data-e2e='comment-list']",
                    "textarea[data-e2e='comment-box']",
                ],
                like_button_selectors: &[
                    "button[data-e2e='like-icon']",
                    "span[data-e2e='like-count']",
                ],
                publish_button_selectors: &["button[data-e2e='post-btn']", "button[type='submit']"],
            },
            SocialPlatform::YouTube => PlatformNavigationMap {
                url: "https://www.youtube.com/",
                logged_in_selectors: &["button#avatar-btn", "ytd-topbar-menu-button-renderer"],
                new_post_selectors: &[
                    "tp-yt-paper-button[aria-label='Create']",
                    "button[aria-label='Create']",
                ],
                comment_section_selectors: &["ytd-comments", "ytd-comment-thread-renderer"],
                like_button_selectors: &[
                    "button[aria-label*='like this video']",
                    "ytd-toggle-button-renderer",
                ],
                publish_button_selectors: &["button[aria-label='Post']", "button[type='submit']"],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformNavigationMap {
    pub url: &'static str,
    pub logged_in_selectors: &'static [&'static str],
    pub new_post_selectors: &'static [&'static str],
    pub comment_section_selectors: &'static [&'static str],
    pub like_button_selectors: &'static [&'static str],
    pub publish_button_selectors: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationResult {
    pub platform: SocialPlatform,
    pub url: String,
    pub logged_in: bool,
    pub new_post_locator: String,
    pub comment_locator: String,
    pub like_locator: String,
    pub publish_locator: String,
    pub used_vision_fallback: bool,
}

pub trait NavigatorBrowser {
    fn open(&mut self, url: &str) -> Result<(), AgentError>;
    fn has_selector(&mut self, selector: &str) -> Result<bool, AgentError>;
}

pub trait NavigatorVision {
    fn check_logged_in(&mut self, platform: SocialPlatform) -> Result<bool, AgentError>;
    fn find_ui_element(
        &mut self,
        platform: SocialPlatform,
        element_name: &str,
    ) -> Result<Option<String>, AgentError>;
}

pub struct PlatformNavigator<B: NavigatorBrowser, V: NavigatorVision> {
    browser: B,
    vision: V,
}

impl<B: NavigatorBrowser, V: NavigatorVision> PlatformNavigator<B, V> {
    pub fn new(browser: B, vision: V) -> Self {
        Self { browser, vision }
    }

    pub fn navigate_to_platform(
        &mut self,
        platform: SocialPlatform,
    ) -> Result<NavigationResult, AgentError> {
        let map = platform.navigation_map();
        self.browser.open(map.url)?;

        let mut used_vision_fallback = false;
        let logged_in = match first_selector_match(&mut self.browser, map.logged_in_selectors)? {
            Some(_) => true,
            None => {
                used_vision_fallback = true;
                self.vision.check_logged_in(platform)?
            }
        };

        let new_post_locator = resolve_locator(
            &mut self.browser,
            &mut self.vision,
            platform,
            "new_post",
            map.new_post_selectors,
            &mut used_vision_fallback,
        )?;
        let comment_locator = resolve_locator(
            &mut self.browser,
            &mut self.vision,
            platform,
            "comment_section",
            map.comment_section_selectors,
            &mut used_vision_fallback,
        )?;
        let like_locator = resolve_locator(
            &mut self.browser,
            &mut self.vision,
            platform,
            "like",
            map.like_button_selectors,
            &mut used_vision_fallback,
        )?;
        let publish_locator = resolve_locator(
            &mut self.browser,
            &mut self.vision,
            platform,
            "publish",
            map.publish_button_selectors,
            &mut used_vision_fallback,
        )?;

        Ok(NavigationResult {
            platform,
            url: map.url.to_string(),
            logged_in,
            new_post_locator,
            comment_locator,
            like_locator,
            publish_locator,
            used_vision_fallback,
        })
    }

    pub fn into_parts(self) -> (B, V) {
        (self.browser, self.vision)
    }
}

fn first_selector_match<B: NavigatorBrowser>(
    browser: &mut B,
    selectors: &[&str],
) -> Result<Option<String>, AgentError> {
    for selector in selectors {
        if browser.has_selector(selector)? {
            return Ok(Some((*selector).to_string()));
        }
    }
    Ok(None)
}

fn resolve_locator<B: NavigatorBrowser, V: NavigatorVision>(
    browser: &mut B,
    vision: &mut V,
    platform: SocialPlatform,
    key: &str,
    selectors: &[&str],
    used_vision_fallback: &mut bool,
) -> Result<String, AgentError> {
    if let Some(selector) = first_selector_match(browser, selectors)? {
        return Ok(selector);
    }

    *used_vision_fallback = true;
    if let Some(locator) = vision.find_ui_element(platform, key)? {
        return Ok(locator);
    }

    Err(AgentError::SupervisorError(format!(
        "unable to resolve '{key}' locator for platform {}",
        platform.as_label()
    )))
}
