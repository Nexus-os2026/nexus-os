use nexus_sdk::errors::AgentError;
use screen_poster_agent::approval::{
    ApprovalDecision, ApprovalError, ApprovedDraft, HumanApprovalGate, InMemoryApprovalChannel,
};
use screen_poster_agent::comments::{Comment, CommentInteractor, ReplyDraft, Sentiment};
use screen_poster_agent::composer::DraftPost;
use screen_poster_agent::navigator::{
    NavigatorBrowser, NavigatorVision, PlatformNavigator, SocialPlatform,
};
use screen_poster_agent::poster::{PostVision, PostingRuntime, ScreenPosterEngine};
use screen_poster_agent::stealth::{gaussian_action_delays_ms_seeded, StealthProfile};

#[derive(Debug, Default)]
struct MockBrowser;

impl NavigatorBrowser for MockBrowser {
    fn open(&mut self, _url: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn has_selector(&mut self, _selector: &str) -> Result<bool, AgentError> {
        Ok(true)
    }
}

#[derive(Debug, Default)]
struct MockNavigatorVision;

impl NavigatorVision for MockNavigatorVision {
    fn check_logged_in(&mut self, _platform: SocialPlatform) -> Result<bool, AgentError> {
        Ok(true)
    }

    fn find_ui_element(
        &mut self,
        _platform: SocialPlatform,
        element_name: &str,
    ) -> Result<Option<String>, AgentError> {
        Ok(Some(format!("vision:{element_name}")))
    }
}

#[derive(Debug, Default)]
struct MockPostingRuntime {
    typed: String,
    screenshots: Vec<String>,
}

impl PostingRuntime for MockPostingRuntime {
    fn click(&mut self, _locator: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn type_char(&mut self, ch: char) -> Result<(), AgentError> {
        self.typed.push(ch);
        Ok(())
    }

    fn upload_media(&mut self, _media_path: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn screenshot(&mut self, label: &str) -> Result<String, AgentError> {
        let id = format!("shot:{label}");
        self.screenshots.push(id.clone());
        Ok(id)
    }
}

#[derive(Debug, Clone)]
struct MockPostVision {
    page_ok: bool,
    preview_ok: bool,
    posted: bool,
}

impl PostVision for MockPostVision {
    fn verify_page(&mut self, _platform: SocialPlatform) -> Result<bool, AgentError> {
        Ok(self.page_ok)
    }

    fn verify_preview(&mut self, _content: &str) -> Result<bool, AgentError> {
        Ok(self.preview_ok)
    }

    fn verify_posted(&mut self) -> Result<bool, AgentError> {
        Ok(self.posted)
    }
}

fn sample_draft(platform: SocialPlatform) -> DraftPost {
    DraftPost {
        platform,
        text: "Hello world from NexusOS".to_string(),
        hashtags: vec!["#nexus".to_string()],
        media_urls: Vec::new(),
        scheduled_time: None,
        variants: vec!["Hello world from NexusOS".to_string()],
    }
}

fn sample_engine(
    posted: bool,
) -> ScreenPosterEngine<MockBrowser, MockNavigatorVision, MockPostingRuntime, MockPostVision> {
    let navigator = PlatformNavigator::new(MockBrowser, MockNavigatorVision);
    let runtime = MockPostingRuntime::default();
    let vision = MockPostVision {
        page_ok: true,
        preview_ok: true,
        posted,
    };
    ScreenPosterEngine::new(navigator, runtime, vision)
}

#[test]
fn test_draft_approval_gate() {
    let mut gate = HumanApprovalGate::new(InMemoryApprovalChannel::default());
    let ticket = gate
        .present_draft(sample_draft(SocialPlatform::X))
        .expect("draft should be presented");

    assert_eq!(gate.channel().desktop_messages.len(), 1);
    assert_eq!(gate.channel().telegram_messages.len(), 1);

    let pending = gate.approved_draft(ticket);
    assert!(matches!(pending, Err(ApprovalError::Pending)));

    gate.decide(ticket, ApprovalDecision::Approve)
        .expect("approval should succeed");
    let approved = gate
        .approved_draft(ticket)
        .expect("approved draft should be retrievable");

    let mut engine = sample_engine(true);
    let result = engine
        .post(&approved, SocialPlatform::X)
        .expect("post should execute after approval");
    assert!(result.posted);
}

#[test]
fn test_human_like_typing() {
    let engine = sample_engine(true);
    let delays = engine.simulate_human_typing("Hello world");

    assert_eq!(delays.len(), "Hello world".chars().count());
    assert!(delays.iter().all(|delay| *delay >= 50 && *delay <= 150));
    let total: u64 = delays.iter().sum();
    assert!(total > 500, "typing should not be instant");
}

#[test]
fn test_anti_detection_delays() {
    let profile = StealthProfile::default();
    let delays = gaussian_action_delays_ms_seeded(10, &profile, 99);

    assert_eq!(delays.len(), 10);
    assert!(delays.iter().all(|delay| *delay >= 500));

    let mean_ms = delays.iter().sum::<u64>() as f64 / delays.len() as f64;
    let mean_secs = mean_ms / 1000.0;
    assert!(
        mean_secs > 1.0 && mean_secs < 3.5,
        "mean delay should remain human-like around 2s, got {mean_secs}"
    );
}

#[test]
fn test_comment_reply_approval() {
    let comment = Comment {
        author: "alex".to_string(),
        text: "How would you deploy this?".to_string(),
        timestamp: "2026-03-03T12:00:00Z".to_string(),
        sentiment: Sentiment::Positive,
    };

    let mut interactor = CommentInteractor::new();
    let reply: ReplyDraft = interactor
        .generate_reply(&comment, "deployment process and rollback strategy")
        .expect("reply draft should be generated");

    let mut gate = HumanApprovalGate::new(InMemoryApprovalChannel::default());
    let ticket = gate
        .present_reply(reply.clone())
        .expect("reply should be sent to approval gate");

    let pending = gate.approved_reply(ticket);
    assert!(matches!(pending, Err(ApprovalError::Pending)));

    gate.decide(ticket, ApprovalDecision::Approve)
        .expect("approval should succeed");
    let approved = gate
        .approved_reply(ticket)
        .expect("reply should become available after approval");
    assert_eq!(approved.reply.comment_author, reply.comment_author);
    assert!(!approved.reply.text.is_empty());
}

#[test]
fn test_post_verification() {
    let approved = ApprovedDraft {
        ticket_id: uuid::Uuid::new_v4(),
        draft: sample_draft(SocialPlatform::X),
    };

    let mut failure_engine = sample_engine(false);
    let failed = failure_engine
        .post(&approved, SocialPlatform::X)
        .expect("post attempt should complete even when verification fails");
    assert!(!failed.posted);

    let mut success_engine = sample_engine(true);
    let success = success_engine
        .post(&approved, SocialPlatform::X)
        .expect("post attempt should complete");
    assert!(success.posted);
}

#[test]
fn test_kill_gate_blocks_post_when_ban_rate_exceeds_threshold() {
    let approved = ApprovedDraft {
        ticket_id: uuid::Uuid::new_v4(),
        draft: sample_draft(SocialPlatform::X),
    };
    let mut engine = sample_engine(true).with_ban_rate_percent(3.0);

    let result = engine.post(&approved, SocialPlatform::X);
    assert!(matches!(result, Err(AgentError::SupervisorError(_))));
}
