# P2-V1-R3B-I4-R2: primary sources

R2 needs no new primary source; it relies on facts already established,
kept apart by kind:

- **Generic D-Bus** (I4-R1, `docs/security/phase2-governed-verification.md`
  section "What the primary sources establish", from the D-Bus
  Specification and NetworkManager's "Notes on D-Bus"; the I4-R1 evidence
  `docs/evidence/p2-v1-r3b-i4-r1-native-scope/`): a method call's effect and
  its reply are independent; a reply can be lost after the recipient acted,
  and a timeout or a broken transport proves nothing about what it did. An
  error reply is a reply like any other: it can be lost too. So a refusal
  (`UnitExists`) can reach the caller as a timeout.
- **Documented systemd behaviour** (upstream v255, byte-identical in
  v255.4):
  - `org.freedesktop.systemd1.UnitExists` is systemd's error for a name
    already loaded (`bus-common-errors.h`,
    `docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/primary/excerpts.md`);
  - StartTransientUnit returns it from `transient_unit_from_message` before
    any property of the request is read: a refused request has no effect
    (`dbus-manager.c` 1016-1018, 1027;
    `docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/primary/excerpts.md`);
  - GetUnit answers with the object path of whatever unit is loaded under
    the name, with no notion of who created it (below): a unit's presence
    under a name is not evidence that this backend's request created it,
    and its absence is not evidence that a still-unhandled request will not.
- **Observed on the host**: nothing. The live suite was not run (the
  runner's user manager is absent, run 37091121444).

## GetUnit

`src/core/dbus-manager.c` (v255), lines 522-541 (sha256 `3630c37996b915a57e786cf8d47b8e6240d9f9922e256dff5febaf1fb65f47b2`, retrieved for P2-V1-R3B-I4-Q1-R1):

```c
  522  static int method_get_unit(sd_bus_message *message, void *userdata, sd_bus_error *error) {
  523          Manager *m = ASSERT_PTR(userdata);
  524          const char *name;
  525          Unit *u;
  526          int r;
  527
  528          assert(message);
  529
  530          /* Anyone can call this method */
  531
  532          r = sd_bus_message_read(message, "s", &name);
  533          if (r < 0)
  534                  return r;
  535
  536          r = bus_get_unit_by_name(m, message, name, &u, error);
  537          if (r < 0)
  538                  return r;
  539
  540          return reply_unit_path(u, message, error);
  541  }
```

The name lookup it uses, lines 455-489:

```c
  455  static int bus_get_unit_by_name(Manager *m, sd_bus_message *message, const char *name, Unit **ret_unit, sd_bus_error *error) {
  456          Unit *u;
  457          int r;
  458
  459          assert(m);
  460          assert(message);
  461          assert(ret_unit);
  462
  463          /* More or less a wrapper around manager_get_unit() that generates nice errors and has one trick up
  464           * its sleeve: if the name is specified empty we use the client's unit. */
  465
  466          if (isempty(name)) {
  467                  _cleanup_(sd_bus_creds_unrefp) sd_bus_creds *creds = NULL;
  468                  pid_t pid;
  469
  470                  r = sd_bus_query_sender_creds(message, SD_BUS_CREDS_PID, &creds);
  471                  if (r < 0)
  472                          return r;
  473
  474                  r = sd_bus_creds_get_pid(creds, &pid);
  475                  if (r < 0)
  476                          return r;
  477
  478                  u = manager_get_unit_by_pid(m, pid);
  479                  if (!u)
  480                          return sd_bus_error_set(error, BUS_ERROR_NO_SUCH_UNIT, "Client not member of any unit.");
  481          } else {
  482                  u = manager_get_unit(m, name);
  483                  if (!u)
  484                          return sd_bus_error_setf(error, BUS_ERROR_NO_SUCH_UNIT, "Unit %s not loaded.", name);
  485          }
  486
  487          *ret_unit = u;
  488          return 0;
  489  }
```
