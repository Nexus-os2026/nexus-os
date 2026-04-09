//! SG5 probe: verifies AT-SPI connection and lists all accessible applications
//! visible in the AT-SPI registry. Diagnostic-only.
//!
//! Run: cargo run --bin sg5_probe
//! (from crates/nexus-ui-repair/)

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::AccessibilityConnection;
use atspi::ObjectRef;

#[tokio::main]
async fn main() {
    println!("SG5: connecting to AT-SPI...");

    let conn = match AccessibilityConnection::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SG5 FAIL: AT-SPI connection failed: {e}");
            eprintln!("Check: gsettings get org.gnome.desktop.interface toolkit-accessibility");
            std::process::exit(1);
        }
    };
    println!("SG5: AT-SPI connection established.");
    let zconn = conn.connection().clone();

    let registry_root = match AccessibleProxy::builder(&zconn)
        .destination("org.a11y.atspi.Registry")
        .and_then(|b| b.path("/org/a11y/atspi/accessible/root"))
    {
        Ok(b) => match b
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("SG5 FAIL: registry root build: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("SG5 FAIL: registry root builder: {e}");
            std::process::exit(1);
        }
    };

    let children: Vec<ObjectRef> = match registry_root.get_children().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SG5 FAIL: get_children: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "SG5: registry has {} top-level applications",
        children.len()
    );
    println!("SG5: application names:");

    for (i, child) in children.into_iter().enumerate() {
        let proxy: AccessibleProxy<'_> = match child.into_accessible_proxy(&zconn).await {
            Ok(p) => p,
            Err(e) => {
                println!("  [{i}] <proxy build failed: {e}>");
                continue;
            }
        };
        let name = proxy.name().await.unwrap_or_else(|_| "<err>".to_string());
        let role = proxy
            .get_role_name()
            .await
            .unwrap_or_else(|_| "<err>".to_string());
        println!("  [{i}] name={name:?} role={role:?}");
    }

    println!("SG5 PASS: AT-SPI registry walked.");
}
