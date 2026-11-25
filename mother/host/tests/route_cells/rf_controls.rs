//! SB-3E CELL-RF-10 (SB3E-R267): the controls of the read_failure carrier.
// Each control is the x:435 cell (registration_record, read_failure) run by
// `s5::cell_with` in a child whose carrier or reader value is wrong, and every
// refusal `witness::credited` would name for it (`witness::refusals`). Then
// the inverse, per driver family: the witnessed shim, never armed, changes no
// control row. Not a section: no credited driver, and no section file named.
use super::{
    reader,
    s5::{cell_with, d_cancel, d_lane, read_registered, registered, Carrier, Facts, Ran},
    store::SHIM_SHA256,
    witness, Fixture,
};
use std::path::PathBuf;

const ROW: (&str, &str) = ("registration_record", "ingress");
const CANCEL: (&str, &str) = ("cancel_run", "ingress");
const LANE: (&str, &str) = ("lane_result_admission", "ingress");
const ID: &str = "rc_rf_registration_record_ingress_read_failure";
const STOPPED: &[Result<(), &str>] = &[
    Err("Backend(\"model_stopped\")"),
    Err("Backend(\"model_cancelled\")"),
];
/// A child that ended before its record: nothing written, nothing witnessed.
const NO_CHILD: [&str; 3] = [
    "record: missing",
    "record: the child failed",
    "record: no shim witness",
];
fn record(f: &Fixture) -> Facts {
    read_registered(f, ROW, ("read_failure", "read_failure"), STOPPED)
}
/// The cell under `carrier`, in the parent: what it saw, and every refusal.
fn refused(
    name: &str,
    drive: fn(&Fixture) -> Facts,
    carrier: Carrier,
) -> Option<(Ran, Vec<String>)> {
    let ran = cell_with(name, ID, drive, witness::render, carrier)?;
    let found = witness::refusals(&ran, ROW.0, ROW.1, "read_failure");
    println!("{name}: shim {:?}; refused {found:?}", ran.shim);
    Some((ran, found))
}
fn has(found: &[String], part: &str) -> bool {
    found.iter().any(|f| f.contains(part))
}

/// The digest in `ROUTE_SHIM_SHA256` is not the library's (store.rs `shim_witness`).
#[test]
fn route_rf_wrong_digest() {
    let carrier: Carrier = |root| {
        let mut env = reader::read_env(root);
        let digest = env.iter_mut().find(|e| e.0 == SHIM_SHA256).unwrap();
        let hex = digest.1.to_string_lossy().into_owned();
        let first = if hex.starts_with('0') { "1" } else { "0" };
        digest.1 = PathBuf::from(format!("{first}{}", &hex[1..]));
        env
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_wrong_digest", record, carrier) else {
        return;
    };
    assert_eq!(found, NO_CHILD, "rf: a child that ended before its record");
    assert!(
        ran.output.contains("assertion `left == right` failed"),
        "{}",
        ran.output
    );
}
/// The library's bytes change after the parent took its digest (a copy in the
/// child's root: the carrier directory is never written).
#[test]
fn route_rf_changed_file() {
    let carrier: Carrier = |root| {
        let carried = reader::read_env(root);
        let library = carried.iter().find(|e| e.0 == "LD_PRELOAD").unwrap();
        let copy = root.join("changed.so");
        let mut bytes = std::fs::read(&library.1).unwrap();
        std::fs::write(&copy, &bytes).unwrap();
        let env = reader::read_carried(root, copy.clone());
        bytes.push(0);
        std::fs::write(&copy, bytes).unwrap();
        env
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_changed_file", record, carrier) else {
        return;
    };
    assert_eq!(found, NO_CHILD, "rf: a child that ended before its record");
    assert!(
        ran.output.contains("assertion `left == right` failed"),
        "{}",
        ran.output
    );
}
/// The preload is not a library and the digest is that file's own: only the
/// /proc/self/maps check refuses it.
#[test]
fn route_rf_not_mapped() {
    let carrier: Carrier = |root| {
        let file = root.join("not_a_library.so");
        std::fs::write(&file, b"not a library\n").unwrap();
        reader::read_carried(root, file)
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_not_mapped", record, carrier) else {
        return;
    };
    assert_eq!(found, NO_CHILD, "rf: a child that ended before its record");
    assert!(
        ran.output.contains("the shim is not mapped"),
        "{}",
        ran.output
    );
}
/// No shim at all: the arm file is made and the reader reads the byte.
#[test]
fn route_rf_shim_absent() {
    let carrier: Carrier = |root| {
        let env = reader::read_env(root).into_iter();
        env.filter(|e| e.0 == reader::ARM).collect()
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_shim_absent", record, carrier) else {
        return;
    };
    assert!(
        ran.shim.is_none() && has(&found, "record: no shim witness"),
        "{found:?}"
    );
    assert!(
        has(&found, "ack=unread[read: branch Data, not Err]"),
        "{found:?}"
    );
}
/// The arm on the wrong read: the reader fails as it opens, before the hold.
#[test]
fn route_rf_wrong_read() {
    let carrier: Carrier = |root| {
        let mut env = reader::read_env(root);
        env.push((reader::EARLY, PathBuf::from("1")));
        env
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_wrong_read", record, carrier) else {
        return;
    };
    let want = "value: expected \"read_failure@model.record.before ack=read won=route\", observed \"unobserved\"";
    assert!(
        ran.shim.is_some() && found.iter().any(|f| f == want),
        "{found:?}"
    );
}
/// A real EOF (Ctrl-D) typed for the read_failure row, the shim witnessed.
#[test]
fn route_rf_typed_eof() {
    let drive = |f: &Fixture| read_registered(f, ROW, ("read_failure", "04"), STOPPED);
    let name = "rf_controls::route_rf_typed_eof";
    let Some((ran, found)) = refused(name, drive, reader::read_env) else {
        return;
    };
    let want = "ack=unread[read: reader value 04, not read_failure]";
    assert!(ran.shim.is_some() && has(&found, want), "{found:?}");
}
/// A witnessed shim whose armed read returns 0: the reader's EOF, not its error.
#[test]
fn route_rf_eof_shim() {
    let carrier: Carrier = |root| {
        let error = "        errno = EIO;\n        return -1;";
        let source = reader::READ_SHIM.replace(error, "        return 0;");
        assert_ne!(source, reader::READ_SHIM, "the shim's error lines");
        reader::read_carried(root, reader::read_built(root, &source))
    };
    let Some((ran, found)) = refused("rf_controls::route_rf_eof_shim", record, carrier) else {
        return;
    };
    let want = "ack=unread[read: branch Zero, not Err]";
    assert!(ran.shim.is_some() && has(&found, want), "{found:?}");
}

/// The inverse: a control row in a child that witnessed the shim and never
/// armed it renders its `CONTROLS` row, byte for byte.
fn unarmed(name: &str, id: &'static str, sink: &str, drive: fn(&Fixture) -> Facts) {
    if let Some(ran) = cell_with(name, id, drive, witness::render, reader::read_env) {
        assert!(ran.shim.is_some(), "{id}: no shim witness\n{}", ran.output);
        witness::control(&ran, sink, "ingress");
    }
}
#[test]
fn route_rf_unarmed_registration() {
    let drive = |f: &Fixture| registered(f, ROW, "none", &[Ok(())]);
    let name = "rf_controls::route_rf_unarmed_registration";
    unarmed(name, "ctl_registration_record", ROW.0, drive);
}
#[test]
fn route_rf_unarmed_cancel_run() {
    let drive = |f: &Fixture| d_cancel(f, CANCEL, "none", None);
    let name = "rf_controls::route_rf_unarmed_cancel_run";
    unarmed(name, "ctl_cancel_run", CANCEL.0, drive);
}
#[test]
fn route_rf_unarmed_lane() {
    let drive = |f: &Fixture| d_lane(f, LANE, ("none", None), ["Ok(())", "none", "Ok(())"]);
    let name = "rf_controls::route_rf_unarmed_lane";
    unarmed(name, "ctl_lane_result_admission", LANE.0, drive);
}
