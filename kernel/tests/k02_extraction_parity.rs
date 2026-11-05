//! K02 parity oracle, registered before the AgentId / FsPermissionLevel
//! extraction it judges. It pins today's behavior through the types' current
//! paths; the extraction must leave every assertion here unchanged (only the
//! import lines follow the types to their new homes).
//!
//! Scope: identity (alias of `Uuid`), Display / serde / byte forms, variant
//! order, the derive set and every serialized form of `FsPermissionLevel`. The
//! grant, narrowing, expiry and revocation behavior stays covered by the
//! unchanged workspace-authority tests; nothing here replaces them.
use nexus_kernel::agent_identity::AgentId;
use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
use nexus_kernel::workspace_authority::WorkspaceBinding;
use serde::de::value::{Error as ValueError, U32Deserializer};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::marker::PhantomData;
use uuid::Uuid;

const AGENT_BYTES: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x46, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const AGENT_TEXT: &str = "00112233-4455-4677-8899-aabbccddeeff";

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Holder<T> {
    value: T,
}

#[test]
fn agent_id_is_the_uuid_type_itself() {
    assert_eq!(TypeId::of::<AgentId>(), TypeId::of::<Uuid>());
    assert_eq!(std::mem::size_of::<AgentId>(), 16);
    // Compile-time proof both directions, with no conversion in between.
    let uuid: Uuid = Uuid::from_bytes(AGENT_BYTES);
    let agent: AgentId = uuid;
    let back: Uuid = agent;
    assert_eq!(back, uuid);
    assert_eq!(agent.as_bytes(), &AGENT_BYTES);
    assert!(AgentId::nil().is_nil());
    let fresh = AgentId::new_v4();
    assert_eq!(fresh.get_version_num(), 4);
    assert_ne!(fresh, AgentId::new_v4());
}

/// The old path is a same-type re-export, not a copy: values cross between the
/// two paths with no conversion. Delete with the old path (K14 retires
/// `supervisor`; K22 requires no old-path exports).
#[test]
fn transitional_agent_id_export_is_the_same_type_as_its_new_home() {
    use nexus_kernel::supervisor::AgentId as OldPathAgentId;
    assert_eq!(TypeId::of::<OldPathAgentId>(), TypeId::of::<AgentId>());
    let new_home: AgentId = Uuid::from_bytes(AGENT_BYTES);
    let old_path: OldPathAgentId = new_home;
    assert_eq!(old_path, new_home);
    assert_eq!(old_path.to_string(), AGENT_TEXT);
}

#[test]
fn agent_id_keeps_display_and_serde_forms() {
    let agent: AgentId = Uuid::from_bytes(AGENT_BYTES);
    assert_eq!(agent.to_string(), AGENT_TEXT);
    assert_eq!(
        serde_json::to_string(&agent).unwrap(),
        format!("\"{AGENT_TEXT}\"")
    );
    assert_eq!(
        serde_json::from_str::<AgentId>(&format!("\"{AGENT_TEXT}\"")).unwrap(),
        agent
    );
    // The existing parse surface is unchanged: the simple form is accepted,
    // and a non-UUID string or a bare number is not authority.
    assert_eq!(
        serde_json::from_str::<AgentId>("\"00112233445546778899aabbccddeeff\"").unwrap(),
        agent
    );
    for bad in [
        "\"\"",
        "\"agent-1\"",
        "\"00112233-4455-4677-8899\"",
        "17",
        "null",
    ] {
        assert!(serde_json::from_str::<AgentId>(bad).is_err(), "{bad}");
    }
    let toml = toml::to_string(&Holder { value: agent }).unwrap();
    assert_eq!(toml, format!("value = \"{AGENT_TEXT}\"\n"));
    assert_eq!(
        toml::from_str::<Holder<AgentId>>(&toml).unwrap().value,
        agent
    );
}

#[test]
fn agent_id_still_feeds_the_workspace_binding_bytes_exactly() {
    let run = [0x42; 16];
    let binding = WorkspaceBinding::from_bytes(AGENT_BYTES, run);
    let agent: AgentId = Uuid::from_bytes(AGENT_BYTES);
    assert_eq!(binding.agent_id, agent);
    assert_eq!(binding.run_id.as_bytes(), &run);
    assert_eq!(binding, WorkspaceBinding::from_bytes(AGENT_BYTES, run));
    // Wrong agent and swapped agent/run bindings are different authority.
    assert_ne!(binding, WorkspaceBinding::from_bytes([0x43; 16], run));
    assert_ne!(binding, WorkspaceBinding::from_bytes(run, AGENT_BYTES));
}

const LEVELS: [(FsPermissionLevel, &str); 3] = [
    (FsPermissionLevel::ReadOnly, "ReadOnly"),
    (FsPermissionLevel::ReadWrite, "ReadWrite"),
    (FsPermissionLevel::Deny, "Deny"),
];

#[test]
fn fs_permission_level_keeps_variant_order() {
    // The derive's variant index is the declaration order. Feeding bare
    // indices through serde proves the order without any new dependency.
    for (index, (expected, _)) in LEVELS.iter().enumerate() {
        let de: U32Deserializer<ValueError> = (index as u32).into_deserializer();
        assert_eq!(&FsPermissionLevel::deserialize(de).unwrap(), expected);
    }
    let out_of_range: U32Deserializer<ValueError> = 3u32.into_deserializer();
    assert!(FsPermissionLevel::deserialize(out_of_range).is_err());
}

#[test]
fn fs_permission_level_keeps_every_serialized_form() {
    for (level, name) in &LEVELS {
        assert_eq!(format!("{level:?}"), *name);
        assert_eq!(serde_json::to_string(level).unwrap(), format!("\"{name}\""));
        assert_eq!(
            &serde_json::from_str::<FsPermissionLevel>(&format!("\"{name}\"")).unwrap(),
            level
        );
        let toml = toml::to_string(&Holder {
            value: level.clone(),
        })
        .unwrap();
        assert_eq!(toml, format!("value = \"{name}\"\n"));
        assert_eq!(
            &toml::from_str::<Holder<FsPermissionLevel>>(&toml)
                .unwrap()
                .value,
            level
        );
        // The only in-tree serialized consumer embeds the level unchanged,
        // after its own `path_pattern` field.
        let entry = FilesystemPermission {
            path_pattern: "/src/**".into(),
            permission: level.clone(),
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            format!("{{\"path_pattern\":\"/src/**\",\"permission\":\"{name}\"}}")
        );
        assert_eq!(
            toml::to_string(&entry).unwrap(),
            format!("path_pattern = \"/src/**\"\npermission = \"{name}\"\n")
        );
    }
    // No spelling beyond the three exact variant names is accepted.
    for bad in [
        "\"read_only\"",
        "\"readonly\"",
        "\"READONLY\"",
        "\"ReadOnly \"",
        "\"Allow\"",
        "\"\"",
        "0",
        "null",
        "[\"ReadOnly\"]",
    ] {
        assert!(
            serde_json::from_str::<FsPermissionLevel>(bad).is_err(),
            "{bad}"
        );
    }
}

// Inherent associated constants win over the blanket fallback only where the
// bound holds, so each flag reports whether the trait is implemented.
struct Probe<T>(PhantomData<T>);
trait Fallback {
    const COPY: bool = false;
    const HASH: bool = false;
    const ORD: bool = false;
    const DEFAULT: bool = false;
}
impl<T> Fallback for Probe<T> {}
impl<T: Copy> Probe<T> {
    const COPY: bool = true;
}
impl<T: std::hash::Hash> Probe<T> {
    const HASH: bool = true;
}
impl<T: Ord> Probe<T> {
    const ORD: bool = true;
}
impl<T: Default> Probe<T> {
    const DEFAULT: bool = true;
}

#[test]
fn fs_permission_level_keeps_exactly_its_derive_set() {
    fn requires<
        T: std::fmt::Debug + Clone + PartialEq + Eq + Serialize + serde::de::DeserializeOwned,
    >() {
    }
    requires::<FsPermissionLevel>();
    let flags = [
        ("Copy", Probe::<FsPermissionLevel>::COPY),
        ("Hash", Probe::<FsPermissionLevel>::HASH),
        ("Ord", Probe::<FsPermissionLevel>::ORD),
        ("Default", Probe::<FsPermissionLevel>::DEFAULT),
    ];
    assert!(
        flags.iter().all(|(_, implemented)| !implemented),
        "{flags:?}"
    );
    // The probe itself is live: it does see traits a type does implement.
    let live = [
        ("Copy", Probe::<u8>::COPY),
        ("Hash", Probe::<u8>::HASH),
        ("Ord", Probe::<u8>::ORD),
        ("Default", Probe::<u8>::DEFAULT),
    ];
    assert!(live.iter().all(|(_, implemented)| *implemented), "{live:?}");
}
