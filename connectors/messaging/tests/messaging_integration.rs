use nexus_connectors_messaging::messaging::MessagingPlatform;
use nexus_connectors_messaging::telegram::TelegramAdapter;
use nexus_connectors_messaging::whatsapp::{WhatsAppAdapter, WhatsAppQualityTier};
use nexus_kernel::errors::AgentError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[test]
fn test_rate_limit_per_platform() {
    let now = Arc::new(AtomicU64::new(0));

    let tg_clock = Arc::clone(&now);
    let mut telegram =
        TelegramAdapter::with_clock(Some(Arc::new(move || tg_clock.load(Ordering::SeqCst))));

    let first = telegram.send_message("chat-1", "hello");
    assert!(first.is_ok());

    let second = telegram.send_message("chat-1", "again");
    assert!(matches!(second, Err(AgentError::SupervisorError(_))));

    now.store(1_001, Ordering::SeqCst);
    let third = telegram.send_message("chat-1", "after window");
    assert!(third.is_ok());

    now.store(2_000, Ordering::SeqCst);
    let wa_clock = Arc::clone(&now);
    let mut whatsapp = WhatsAppAdapter::with_clock(
        WhatsAppQualityTier::Low,
        Some(Arc::new(move || wa_clock.load(Ordering::SeqCst))),
    );

    let wa_first = whatsapp.send_message("chat-2", "workflow update");
    assert!(wa_first.is_ok());

    let wa_second = whatsapp.send_message("chat-2", "second immediate");
    assert!(matches!(wa_second, Err(AgentError::SupervisorError(_))));
}
