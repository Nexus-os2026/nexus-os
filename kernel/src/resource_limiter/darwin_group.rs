//! Private group observation used only after an owned, unreaped root exited
//! and Darwin denied SIGKILL. The decision/query mechanics also run in Linux
//! unit tests; no Darwin syscall or execution behavior is added on Linux.
use nix::errno::Errno;

const ATTEMPTS: usize = 3;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Live,
    Zombie,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Member {
    pid: i32,
    pgid: i32,
    state: State,
    started: (i64, i64),
}

fn terminal_snapshot(
    pgid: i32,
    snapshot: Result<Vec<Member>, Errno>,
) -> Result<Vec<Member>, Errno> {
    let mut members = snapshot?;
    members.sort_unstable_by_key(|member| member.pid);
    for (index, member) in members.iter().enumerate() {
        if member.pid <= 0
            || member.pgid != pgid
            || member.started.0 < 0
            || !(0..1_000_000).contains(&member.started.1)
            || (index > 0 && members[index - 1].pid == member.pid)
        {
            return Err(Errno::EINVAL);
        }
        match member.state {
            State::Zombie => {}
            State::Live => return Err(Errno::EPERM),
            State::Unknown => return Err(Errno::EINVAL),
        }
    }
    Ok(members)
}

fn complete_membership(members: &[Member], bytes: &[u8]) -> Result<(), Errno> {
    if !bytes.len().is_multiple_of(size_of::<i32>()) {
        return Err(Errno::EINVAL);
    }
    let mut ids: Vec<_> = bytes
        .chunks_exact(size_of::<i32>())
        .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("whole PID record")))
        .collect();
    ids.sort_unstable();
    if ids.iter().any(|pid| *pid <= 0) || ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Errno::EINVAL);
    }
    let mut observed: Vec<_> = members.iter().map(|member| member.pid).collect();
    observed.sort_unstable();
    if ids != observed {
        return Err(Errno::EAGAIN);
    }
    Ok(())
}

fn confirm_snapshots(
    pgid: i32,
    mut observe: impl FnMut() -> Result<Vec<Member>, Errno>,
) -> Result<(), Errno> {
    let first = terminal_snapshot(pgid, observe())?;
    let second = terminal_snapshot(pgid, observe())?;
    // Reject observed membership/identity changes, including disappearing
    // zombies, instead of treating a racing observation as terminal proof.
    if first != second {
        return Err(Errno::EAGAIN);
    }
    Ok(())
}

fn read_records(
    record_size: usize,
    mut query: impl FnMut(Option<&mut [u8]>) -> Result<usize, Errno>,
) -> Result<Vec<u8>, Errno> {
    if record_size == 0 || record_size > MAX_BYTES {
        return Err(Errno::EINVAL);
    }
    for _ in 0..ATTEMPTS {
        let size = query(None)?;
        if size > MAX_BYTES {
            return Err(Errno::EOVERFLOW);
        }
        if !size.is_multiple_of(record_size) {
            return Err(Errno::EINVAL);
        }
        // Even a zero size requires a real fetch: a null oldp would only
        // repeat the size query and could miss members created in between.
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size.max(record_size))
            .map_err(|_| Errno::ENOMEM)?;
        bytes.resize(size.max(record_size), 0);
        match query(Some(&mut bytes)) {
            Err(Errno::ENOMEM) => continue, // never inspect partial results
            Err(error) => return Err(error),
            Ok(written) => {
                if written > bytes.len() || !written.is_multiple_of(record_size) {
                    return Err(Errno::EINVAL);
                }
                bytes.truncate(written);
                return Ok(bytes);
            }
        }
    }
    Err(Errno::ENOMEM)
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use nix::libc;

    #[repr(C)]
    #[derive(Default)]
    struct RawMember {
        pid: i32,
        pgid: i32,
        state: i32,
        start_seconds: i64,
        start_microseconds: i64,
    }

    unsafe extern "C" {
        fn nexus_darwin_proc_record_size() -> usize;
        fn nexus_darwin_group_pids(
            pgid: i32,
            buffer: *mut libc::c_void,
            capacity: usize,
            written: *mut usize,
        ) -> libc::c_int;
        fn nexus_darwin_proc_decode(
            bytes: *const u8,
            length: usize,
            index: usize,
            out: *mut RawMember,
        ) -> libc::c_int;
    }

    fn state(value: i32) -> State {
        match u32::try_from(value) {
            Ok(libc::SZOMB) => State::Zombie,
            Ok(libc::SIDL | libc::SRUN | libc::SSLEEP | libc::SSTOP) => State::Live,
            _ => State::Unknown,
        }
    }

    pub(super) fn snapshot(pgid: i32) -> Result<Vec<Member>, Errno> {
        if pgid <= 0 {
            return Err(Errno::EINVAL);
        }
        // SAFETY: the SDK shim returns sizeof(kinfo_proc), not an assumed ABI.
        let record_size = unsafe { nexus_darwin_proc_record_size() };
        let bytes = read_records(record_size, |buffer| {
            let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PGRP, pgid];
            let (output, mut length) = match buffer {
                Some(bytes) => (bytes.as_mut_ptr().cast(), bytes.len()),
                None => (std::ptr::null_mut(), 0),
            };
            // SAFETY: valid MIB, initialized byte allocation of length bytes,
            // writable size_t, and no new-value pointer (read-only query).
            let result = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as _,
                    output,
                    &mut length,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if result == -1 {
                Err(Errno::last())
            } else {
                Ok(length)
            }
        })?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(bytes.len() / record_size)
            .map_err(|_| Errno::ENOMEM)?;
        for index in 0..bytes.len() / record_size {
            let mut member = RawMember::default();
            // SAFETY: complete validated record buffer and matching repr(C)
            // output. The shim memcpy avoids unaligned structure access.
            let error = unsafe {
                nexus_darwin_proc_decode(bytes.as_ptr(), bytes.len(), index, &mut member)
            };
            if error != 0 {
                return Err(Errno::from_raw(error));
            }
            members.push(Member {
                pid: member.pid,
                pgid: member.pgid,
                state: state(member.state),
                started: (member.start_seconds, member.start_microseconds),
            });
        }
        // sysctl's process iterator omits SIDL and can race with fork/reap.
        // Require its records to cover the subsequent complete group list,
        // captured by libproc while the kernel holds proc_list_lock.
        let ids = read_records(size_of::<i32>(), |buffer| {
            let (output, capacity) = match buffer {
                Some(bytes) => (bytes.as_mut_ptr().cast(), bytes.len()),
                None => (std::ptr::null_mut(), 0),
            };
            let mut written = 0;
            // SAFETY: optional writable allocation of capacity bytes and valid
            // length pointer; the shim preserves errors and rejects truncation.
            let error = unsafe { nexus_darwin_group_pids(pgid, output, capacity, &mut written) };
            if error != 0 {
                Err(Errno::from_raw(error))
            } else {
                Ok(written)
            }
        })?;
        complete_membership(&members, &ids)?;
        Ok(members)
    }

    #[cfg(test)]
    #[test]
    fn sdk_status_constants_are_classified() {
        assert_eq!(state(libc::SZOMB as i32), State::Zombie);
        for value in [libc::SIDL, libc::SRUN, libc::SSLEEP, libc::SSTOP] {
            assert_eq!(state(value as i32), State::Live);
        }
        assert_eq!(state(0), State::Unknown);
        assert_eq!(state(-1), State::Unknown);
    }
}

#[cfg(target_os = "macos")]
pub(super) fn confirm_terminal(pgid: i32) -> Result<(), Errno> {
    confirm_snapshots(pgid, || native::snapshot(pgid))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(pid: i32, state: State) -> Member {
        Member {
            pid,
            pgid: 100,
            state,
            started: (1, 0),
        }
    }

    #[test]
    fn empty_and_zombie_only_groups_allow_finalization() {
        for group in [
            vec![],
            vec![member(100, State::Zombie)],
            vec![member(100, State::Zombie), member(101, State::Zombie)],
        ] {
            assert_eq!(confirm_snapshots(100, || Ok(group.clone())), Ok(()));
        }
    }

    #[test]
    fn live_descendants_preserve_original_eperm_even_after_root_exit() {
        for group in [
            vec![member(101, State::Live)],
            vec![member(100, State::Zombie), member(101, State::Live)],
        ] {
            assert_eq!(
                confirm_snapshots(100, || Ok(group.clone())),
                Err(Errno::EPERM)
            );
        }
    }

    #[test]
    fn unknown_state_and_invalid_records_fail_closed() {
        let mut wrong_group = member(101, State::Zombie);
        wrong_group.pgid = 200;
        for group in [
            vec![member(100, State::Unknown)],
            vec![wrong_group],
            vec![member(0, State::Zombie)],
            vec![member(100, State::Zombie), member(100, State::Zombie)],
        ] {
            assert_eq!(
                confirm_snapshots(100, || Ok(group.clone())),
                Err(Errno::EINVAL)
            );
        }
    }

    #[test]
    fn observation_failures_are_preserved_on_either_snapshot() {
        for error in [Errno::EPERM, Errno::EACCES, Errno::EIO, Errno::ESRCH] {
            assert_eq!(confirm_snapshots(100, || Err(error)), Err(error));
            let mut calls = 0;
            assert_eq!(
                confirm_snapshots(100, || {
                    calls += 1;
                    if calls == 1 {
                        Ok(vec![])
                    } else {
                        Err(error)
                    }
                }),
                Err(error)
            );
        }
    }

    #[test]
    fn changed_membership_or_identity_fails_closed() {
        for second in [
            vec![],
            vec![Member {
                started: (2, 0),
                ..member(100, State::Zombie)
            }],
            vec![member(100, State::Zombie), member(101, State::Zombie)],
        ] {
            let mut calls = 0;
            assert_eq!(
                confirm_snapshots(100, || {
                    calls += 1;
                    Ok(if calls == 1 {
                        vec![member(100, State::Zombie)]
                    } else {
                        second.clone()
                    })
                }),
                Err(Errno::EAGAIN)
            );
        }
    }

    #[test]
    fn empty_size_still_fetches_and_can_find_a_live_record() {
        let bytes = read_records(4, |buffer| match buffer {
            None => Ok(0),
            Some(bytes) => {
                bytes.copy_from_slice(&[1, 2, 3, 4]);
                Ok(4)
            }
        })
        .unwrap();
        assert_eq!(bytes, [1, 2, 3, 4]);
    }

    #[test]
    fn malformed_sizes_and_oversized_results_are_rejected() {
        assert_eq!(read_records(0, |_| Ok(0)), Err(Errno::EINVAL));
        assert_eq!(read_records(4, |_| Ok(3)), Err(Errno::EINVAL));
        assert_eq!(
            read_records(4, |_| Ok(MAX_BYTES + 4)),
            Err(Errno::EOVERFLOW)
        );
        for written in [3, 8] {
            assert_eq!(
                read_records(4, |buffer| Ok(if buffer.is_none() { 4 } else { written })),
                Err(Errno::EINVAL)
            );
        }
    }

    #[test]
    fn growth_retries_are_bounded_and_partial_bytes_are_discarded() {
        let mut fetches = 0;
        assert_eq!(
            read_records(4, |buffer| match buffer {
                None => Ok(4),
                Some(_) => {
                    fetches += 1;
                    Err(Errno::ENOMEM)
                }
            }),
            Err(Errno::ENOMEM)
        );
        assert_eq!(fetches, ATTEMPTS);
        let mut fetches = 0;
        let bytes = read_records(4, |buffer| match buffer {
            None => Ok(4),
            Some(bytes) => {
                fetches += 1;
                if fetches == 1 {
                    bytes.fill(9);
                    Err(Errno::ENOMEM)
                } else {
                    bytes.fill(2);
                    Ok(4)
                }
            }
        })
        .unwrap();
        assert_eq!(bytes, [2; 4]);
        assert_eq!(fetches, 2);
    }

    #[test]
    fn missing_or_new_group_members_cannot_be_accepted() {
        let records = [member(100, State::Zombie)];
        assert_eq!(
            complete_membership(&records, &100_i32.to_ne_bytes()),
            Ok(())
        );
        assert_eq!(complete_membership(&[], &[]), Ok(()));
        let mut ids = 100_i32.to_ne_bytes().to_vec();
        ids.extend_from_slice(&101_i32.to_ne_bytes());
        assert_eq!(complete_membership(&records, &ids), Err(Errno::EAGAIN));
        assert_eq!(complete_membership(&records, &[]), Err(Errno::EAGAIN));
        assert_eq!(complete_membership(&records, &[0; 3]), Err(Errno::EINVAL));
        assert_eq!(
            complete_membership(&records, &0_i32.to_ne_bytes()),
            Err(Errno::EINVAL)
        );
        let duplicate = 100_i32.to_ne_bytes().repeat(2);
        assert_eq!(
            complete_membership(&records, &duplicate),
            Err(Errno::EINVAL)
        );
    }

    #[test]
    fn query_errors_are_not_empty_snapshots() {
        for error in [Errno::EPERM, Errno::EACCES, Errno::EIO] {
            assert_eq!(read_records(4, |_| Err(error)), Err(error));
            assert_eq!(
                read_records(4, |buffer| if buffer.is_none() {
                    Ok(4)
                } else {
                    Err(error)
                }),
                Err(error)
            );
        }
    }
}
