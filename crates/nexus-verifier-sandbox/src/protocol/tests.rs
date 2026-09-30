use super::*;

fn launch() -> LaunchMessage {
    LaunchMessage {
        generation: 7,
        policy_hash: [3; 32],
        argv: vec![b"cargo".to_vec(), b"test".to_vec()],
        env: vec![b"HOME=/w/home".to_vec(), b"LC_ALL=C".to_vec()],
        fds: vec![
            FdRole::Executable,
            FdRole::WorkingDirectory,
            FdRole::Rule(Role::ToolchainRoot),
            FdRole::Rule(Role::CandidateInput),
            FdRole::Rule(Role::DevUrandom),
        ],
    }
}

#[test]
fn p2c_protocol_messages_round_trip_exactly() {
    let messages = [ToHelper::Launch(launch()), ToHelper::Mapped];
    for message in messages {
        assert_eq!(ToHelper::decode(&message.encode()), Ok(message.clone()));
    }
    let replies = [
        FromHelper::Hello {
            version: PROTOCOL_VERSION,
            policy_hash: [9; 32],
        },
        FromHelper::NamespacesReady,
        FromHelper::Running,
        FromHelper::SetupFailed {
            stage: SetupStage::Landlock,
            errno: 95,
        },
        FromHelper::Finished(VerifierStatus::Exited(101)),
        FromHelper::Finished(VerifierStatus::Signalled(9)),
        FromHelper::InitLost,
    ];
    for reply in replies {
        assert_eq!(FromHelper::decode(&reply.encode()), Ok(reply.clone()));
    }
    for role in Role::ALL {
        assert_eq!(
            role_from_code(role_code(FdRole::Rule(role))),
            Some(FdRole::Rule(role))
        );
    }
}

#[test]
fn p2c_truncated_trailing_empty_and_unknown_messages_are_refused() {
    let bytes = ToHelper::Launch(launch()).encode();
    for cut in 1..bytes.len() {
        assert!(ToHelper::decode(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(ToHelper::decode(&trailing), Err(DecodeError::TrailingBytes));
    assert_eq!(ToHelper::decode(&[]), Err(DecodeError::Empty));
    assert_eq!(ToHelper::decode(&[99]), Err(DecodeError::UnknownTag));
    assert_eq!(FromHelper::decode(&[99]), Err(DecodeError::UnknownTag));
    assert_eq!(
        ToHelper::decode(&vec![TAG_MAPPED; MAX_MESSAGE_BYTES + 1]),
        Err(DecodeError::TooLarge)
    );
    let mut wrong_version = bytes;
    wrong_version[1..5].copy_from_slice(&2u32.to_be_bytes());
    assert_eq!(ToHelper::decode(&wrong_version), Err(DecodeError::Invalid));
    assert_eq!(
        FromHelper::decode(&[TAG_SETUP_FAILED, 200, 0, 0, 0, 0]),
        Err(DecodeError::Invalid)
    );
}

#[test]
fn p2c_oversized_counts_and_entries_are_refused_before_allocation() {
    let mut huge = vec![TAG_LAUNCH];
    huge.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    huge.extend_from_slice(&0u64.to_be_bytes());
    huge.extend_from_slice(&[0; 32]);
    huge.extend_from_slice(&u32::MAX.to_be_bytes()); // argc
    assert_eq!(ToHelper::decode(&huge), Err(DecodeError::TooLarge));

    let mut message = launch();
    message.argv = vec![b"x".to_vec(); MAX_ARGS + 1];
    assert!(ToHelper::decode(&ToHelper::Launch(message).encode()).is_err());
    let mut message = launch();
    message.argv[1] = vec![b'a'; MAX_ENTRY_BYTES + 1];
    assert!(ToHelper::decode(&ToHelper::Launch(message).encode()).is_err());
    let mut message = launch();
    message.fds = vec![FdRole::Rule(Role::Scratch); MAX_FDS + 1];
    assert!(ToHelper::decode(&ToHelper::Launch(message).encode()).is_err());
}

#[test]
fn p2c_malformed_launch_content_is_refused() {
    let refused = |edit: &dyn Fn(&mut LaunchMessage)| {
        let mut message = launch();
        edit(&mut message);
        assert!(!message.is_well_formed(), "{message:?}");
        assert_eq!(
            ToHelper::decode(&ToHelper::Launch(message).encode()),
            Err(DecodeError::Invalid)
        );
    };
    refused(&|m| m.argv.clear());
    refused(&|m| m.argv[0] = b"car\0go".to_vec());
    refused(&|m| m.env.push(b"=value".to_vec()));
    refused(&|m| m.env.push(b"NOEQUALS".to_vec()));
    refused(&|m| m.env.push(b"lower=case".to_vec()));
    refused(&|m| m.env.push(b"HOME=/elsewhere".to_vec()));
    refused(&|m| m.env.push(b"NUL=a\0b".to_vec()));
    refused(&|m| m.fds.retain(|role| *role != FdRole::Executable));
    refused(&|m| m.fds.push(FdRole::Executable));
    refused(&|m| m.fds.retain(|role| *role != FdRole::WorkingDirectory));
    refused(&|m| m.fds.push(FdRole::WorkingDirectory));

    let mut bytes = ToHelper::Launch(launch()).encode();
    let last = bytes.len() - 1;
    bytes[last] = 250; // no such role
    assert_eq!(ToHelper::decode(&bytes), Err(DecodeError::Invalid));
    assert_eq!(role_from_code(0), None);
    assert_eq!(role_from_code(3), None);
}
