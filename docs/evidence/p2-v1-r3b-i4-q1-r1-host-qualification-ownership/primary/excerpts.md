# P2-V1-R3B-I4-Q1-R1: primary-source excerpts, verbatim

The repaired collision qualification (H6) sends one request of its own: a
`StartTransientUnit` that names the unit of a scope the production owner
has proven and still holds, and that carries no process and no property.
This file holds the primary sources its safety rests on, verbatim with line
numbers, each file identified by its SHA-256 (`retrieval.txt`). The
excerpts are kept apart by what they can establish:

- **Generic D-Bus** (unchanged from I4-R1 and Q1,
  `docs/evidence/p2-v1-r3b-i4-r1-native-scope/baseline/`): a call's effect
  and its reply are independent; a timeout or a broken transport proves
  nothing about what the recipient did or will do; a recipient need not
  handle concurrent calls in the order they were sent. Nothing here relies
  on an ordering fence.
- **Documented systemd behavior** (upstream source, tag v255; the host runs
  Ubuntu's 255.4-1ubuntu8.17: every function quoted below is byte-identical
  in systemd-stable v255.4, `retrieval.txt`; Ubuntu's own patches were not
  inspected): what the manager does with such a request, whenever it
  handles it.
- **Library and toolchain behavior** (zbus 4.4.0 from crates.io as locked;
  Rust 1.94.0's standard library): for the H1 review.
- **What is observed on the host**: nothing in this mission. The live suite
  was not run (the runner's user manager is absent, run 37091121444); the
  exact answer `org.freedesktop.systemd1.UnitExists` on the real host stays
  a live-gate fact (H6).

## What the excerpts establish for the request without processes

1. A start for the name of a loaded transient unit is refused with
   `BUS_ERROR_UNIT_EXISTS` (`org.freedesktop.systemd1.UnitExists`,
   `bus-common-errors.h`, Q1's excerpts) by `transient_unit_from_message`
   (`dbus-manager.c` 1016-1018) *before* `bus_unit_set_properties` reads any
   of the request's properties (1027): a refused request has no effect.
2. A transient unit is never pristine while it is loaded: `unit_make_transient`
   sets its `fragment_path` to its transient file (`unit.c` 4750), and
   `unit_is_pristine` requires `!u->fragment_path` (5165-5169). Every loaded
   scope is transient (`scope_load` refuses a non-transient one outside a
   reload, `scope.c` 190-192), so a start for a loaded scope's name is
   always refused as above.
3. A scope's processes come only from its `PIDs` and `PIDFDs` properties
   (`dbus-scope.c` 86, 142). The request carries no property at all, so it
   names no process.
4. Handled when no unit of the name is loaded (for example, late, after the
   proven scope has been released), the request would create a transient
   scope without processes: `scope_load` ends in `scope_verify`, which
   refuses a scope with no PIDs (`scope.c` 133-136, "Scope has no PIDs.
   Refusing."); the failed load leaves the unit `UNIT_ERROR` (`unit.c`
   1734-1736); a start job for a unit that did not load is refused
   (`transaction.c` 962-986, `bus_unit_validate_load_state`, `dbus-unit.c`
   2528-2556); nothing is started and no process is attached. Freeing the
   unloaded unit removes its transient file (`unit.c` 758-759, 665-690).
5. Hence, whatever the request is answered and whenever it is handled (a
   timeout proves nothing), it cannot place a process anywhere or change the
   loaded unit, and it needs no authority over any unit. The refusal in 1
   depends on the named unit having a fragment path: for a name loaded
   without one, `unit_is_pristine` alone would not refuse the request
   (`unit.c` 5165-5169). The probe never names such a unit: it names only
   the proven scope's unit, a transient scope created by the production
   owner's own request (pinned by `p2_g_11`), never a name it chose.
6. Even a manager that did not conform (one that loaded a scope without
   processes) could only leave a unit holding no process, under a verifier
   scope name: the checked cleanup observation (`scopes_back`, and the
   gate's observation after the suite) reports it as left over, and nothing
   sweeps it by name: the case fails.

## H1: the bus a normal build connects to, and the environment

- `ScopeManager::connect` formats `/run/user/<uid>/bus` from `getuid` and
  hands it to `ZbusManager::connect_at`, which checks the socket's type and
  owner and connects with `zbus::connection::Builder::address("unix:path=…")`
  (pinned by `i4q1r1_connect_derives_the_bus_from_the_real_uid_alone`).
- zbus 4.4.0's `Builder::address` builds its target from the address it is
  given; only `Builder::session`/`Builder::system` (through
  `Address::session`/`Address::system`) read `DBUS_SESSION_BUS_ADDRESS`,
  `XDG_RUNTIME_DIR` or `DBUS_SYSTEM_BUS_ADDRESS` (below). zbus also reads
  `FLATPAK_ID` (`utils.rs` 53-55) to choose its handshake's pipelining and
  to serialize message creation: never which bus it connects to.
- Rust 1.94.0's own contract for `set_var` and `remove_var` (below): in a
  multi-threaded program on Linux "the only safe option is to not use
  `set_var` or `remove_var` at all"; from the 2024 edition both are
  `unsafe`. The live harness is multi-threaded when H1 runs
  (`matrices/h1-environment-review.md`).

## The excerpts

### StartTransientUnit refuses a loaded unit's name before it reads any property

`src/core/dbus-manager.c (v255; identical in v255.4)`, lines 987-1045 (sha256 `3630c37996b915a57e786cf8d47b8e6240d9f9922e256dff5febaf1fb65f47b2`):

```c
  987  static int transient_unit_from_message(
  988                  Manager *m,
  989                  sd_bus_message *message,
  990                  const char *name,
  991                  Unit **unit,
  992                  sd_bus_error *error) {
  993
  994          UnitType t;
  995          Unit *u;
  996          int r;
  997
  998          assert(m);
  999          assert(message);
 1000          assert(name);
 1001
 1002          t = unit_name_to_type(name);
 1003          if (t < 0)
 1004                  return sd_bus_error_setf(error, SD_BUS_ERROR_INVALID_ARGS,
 1005                                           "Invalid unit name or type.");
 1006
 1007          if (!unit_vtable[t]->can_transient)
 1008                  return sd_bus_error_setf(error, SD_BUS_ERROR_INVALID_ARGS,
 1009                                           "Unit type %s does not support transient units.",
 1010                                           unit_type_to_string(t));
 1011
 1012          r = manager_load_unit(m, name, NULL, error, &u);
 1013          if (r < 0)
 1014                  return r;
 1015
 1016          if (!unit_is_pristine(u))
 1017                  return sd_bus_error_setf(error, BUS_ERROR_UNIT_EXISTS,
 1018                                           "Unit %s was already loaded or has a fragment file.", name);
 1019
 1020          /* OK, the unit failed to load and is unreferenced, now let's
 1021           * fill in the transient data instead */
 1022          r = unit_make_transient(u);
 1023          if (r < 0)
 1024                  return r;
 1025
 1026          /* Set our properties */
 1027          r = bus_unit_set_properties(u, message, UNIT_RUNTIME, false, error);
 1028          if (r < 0)
 1029                  return r;
 1030
 1031          /* If the client asked for it, automatically add a reference to this unit. */
 1032          if (u->bus_track_add) {
 1033                  r = bus_unit_track_add_sender(u, message);
 1034                  if (r < 0)
 1035                          return log_error_errno(r, "Failed to watch sender: %m");
 1036          }
 1037
 1038          /* Now load the missing bits of the unit we just created */
 1039          unit_add_to_load_queue(u);
 1040          manager_dispatch_load_queue(m);
 1041
 1042          *unit = u;
 1043
 1044          return 0;
 1045  }
```

### The start request's handler: the transient unit, then its start job

`src/core/dbus-manager.c (v255)`, lines 1087-1124 (sha256 `3630c37996b915a57e786cf8d47b8e6240d9f9922e256dff5febaf1fb65f47b2`):

```c
 1087  static int method_start_transient_unit(sd_bus_message *message, void *userdata, sd_bus_error *error) {
 1088          const char *name, *smode;
 1089          Manager *m = ASSERT_PTR(userdata);
 1090          JobMode mode;
 1091          Unit *u;
 1092          int r;
 1093
 1094          assert(message);
 1095
 1096          r = mac_selinux_access_check(message, "start", error);
 1097          if (r < 0)
 1098                  return r;
 1099
 1100          r = sd_bus_message_read(message, "ss", &name, &smode);
 1101          if (r < 0)
 1102                  return r;
 1103
 1104          mode = job_mode_from_string(smode);
 1105          if (mode < 0)
 1106                  return sd_bus_error_setf(error, SD_BUS_ERROR_INVALID_ARGS, "Job mode %s is invalid.", smode);
 1107
 1108          r = bus_verify_manage_units_async(m, message, error);
 1109          if (r < 0)
 1110                  return r;
 1111          if (r == 0)
 1112                  return 1; /* No authorization for now, but the async polkit stuff will call us again when it has it */
 1113
 1114          r = transient_unit_from_message(m, message, name, &u, error);
 1115          if (r < 0)
 1116                  return r;
 1117
 1118          r = transient_aux_units_from_message(m, message, error);
 1119          if (r < 0)
 1120                  return r;
 1121
 1122          /* Finally, start it */
 1123          return bus_unit_queue_job(message, u, JOB_START, mode, 0, error);
 1124  }
```

### A loaded transient unit is never pristine

`src/core/unit.c (v255)`, lines 5153-5170 (sha256 `d3c14e765dceeafd8e2d1fe94ed685c5dfc1c35f0a32c8459046a0459a1c57c4`):

```c
 5153  bool unit_is_pristine(Unit *u) {
 5154          assert(u);
 5155
 5156          /* Check if the unit already exists or is already around, in a number of different ways. Note that to
 5157           * cater for unit types such as slice, we are generally fine with units that are marked UNIT_LOADED
 5158           * even though nothing was actually loaded, as those unit types don't require a file on disk.
 5159           *
 5160           * Note that we don't check for drop-ins here, because we allow drop-ins for transient units
 5161           * identically to non-transient units, both unit-specific and hierarchical. E.g. for a-b-c.service:
 5162           * service.d/….conf, a-.service.d/….conf, a-b-.service.d/….conf, a-b-c.service.d/….conf.
 5163           */
 5164
 5165          return IN_SET(u->load_state, UNIT_NOT_FOUND, UNIT_LOADED) &&
 5166                 !u->fragment_path &&
 5167                 !u->source_path &&
 5168                 !u->job &&
 5169                 !u->merged_into;
 5170  }
```

### A transient unit carries its fragment path from creation

`src/core/unit.c (v255)`, lines 4723-4767 (sha256 `d3c14e765dceeafd8e2d1fe94ed685c5dfc1c35f0a32c8459046a0459a1c57c4`):

```c
 4723  int unit_make_transient(Unit *u) {
 4724          _cleanup_free_ char *path = NULL;
 4725          FILE *f;
 4726
 4727          assert(u);
 4728
 4729          if (!UNIT_VTABLE(u)->can_transient)
 4730                  return -EOPNOTSUPP;
 4731
 4732          (void) mkdir_p_label(u->manager->lookup_paths.transient, 0755);
 4733
 4734          path = path_join(u->manager->lookup_paths.transient, u->id);
 4735          if (!path)
 4736                  return -ENOMEM;
 4737
 4738          /* Let's open the file we'll write the transient settings into. This file is kept open as long as we are
 4739           * creating the transient, and is closed in unit_load(), as soon as we start loading the file. */
 4740
 4741          WITH_UMASK(0022) {
 4742                  f = fopen(path, "we");
 4743                  if (!f)
 4744                          return -errno;
 4745          }
 4746
 4747          safe_fclose(u->transient_file);
 4748          u->transient_file = f;
 4749
 4750          free_and_replace(u->fragment_path, path);
 4751
 4752          u->source_path = mfree(u->source_path);
 4753          u->dropin_paths = strv_free(u->dropin_paths);
 4754          u->fragment_mtime = u->source_mtime = u->dropin_mtime = 0;
 4755
 4756          u->load_state = UNIT_STUB;
 4757          u->load_error = 0;
 4758          u->transient = true;
 4759
 4760          unit_add_to_dbus_queue(u);
 4761          unit_add_to_gc_queue(u);
 4762
 4763          fputs("# This is a transient unit file, created programmatically via the systemd API. Do not edit.\n",
 4764                u->transient_file);
 4765
 4766          return 0;
 4767  }
```

### A scope's processes come only from its PIDs and PIDFDs properties

`src/core/dbus-scope.c (v255)`, lines 86-90 (sha256 `087c79760becc4784d78a146cdb60c7101fa1b3f4afde4b1429ca70880772de1`):

```c
   86          if (streq(name, "PIDs")) {
   87                  _cleanup_(sd_bus_creds_unrefp) sd_bus_creds *creds = NULL;
   88                  unsigned n = 0;
   89
   90                  r = sd_bus_message_enter_container(message, 'a', "u");
```

### (PIDFDs)

`src/core/dbus-scope.c (v255)`, lines 142-147 (sha256 `087c79760becc4784d78a146cdb60c7101fa1b3f4afde4b1429ca70880772de1`):

```c
  142          if (streq(name, "PIDFDs")) {
  143                  unsigned n = 0;
  144
  145                  r = sd_bus_message_enter_container(message, 'a', "h");
  146                  if (r < 0)
  147                          return r;
```

### Loading a scope: refused unless transient; verified

`src/core/scope.c (v255; identical in v255.4)`, lines 183-210 (sha256 `b743da9a7511066758ff15c487992baca80d0df3d95f5957fbbee78aaf6ec788`):

```c
  183  static int scope_load(Unit *u) {
  184          Scope *s = SCOPE(u);
  185          int r;
  186
  187          assert(s);
  188          assert(u->load_state == UNIT_STUB);
  189
  190          if (!u->transient && !MANAGER_IS_RELOADING(u->manager))
  191                  /* Refuse to load non-transient scope units, but allow them while reloading. */
  192                  return -ENOENT;
  193
  194          r = scope_load_init_scope(u);
  195          if (r < 0)
  196                  return r;
  197
  198          r = unit_load_fragment_and_dropin(u, false);
  199          if (r < 0)
  200                  return r;
  201
  202          if (u->load_state != UNIT_LOADED)
  203                  return 0;
  204
  205          r = scope_add_extras(s);
  206          if (r < 0)
  207                  return r;
  208
  209          return scope_verify(s);
  210  }
```

### A scope without processes is refused when it is loaded

`src/core/scope.c (v255)`, lines 129-139 (sha256 `b743da9a7511066758ff15c487992baca80d0df3d95f5957fbbee78aaf6ec788`):

```c
  129  static int scope_verify(Scope *s) {
  130          assert(s);
  131          assert(UNIT(s)->load_state == UNIT_LOADED);
  132
  133          if (set_isempty(UNIT(s)->pids) &&
  134              !MANAGER_IS_RELOADING(UNIT(s)->manager) &&
  135              !unit_has_name(UNIT(s), SPECIAL_INIT_SCOPE))
  136                  return log_unit_error_errno(UNIT(s), SYNTHETIC_ERRNO(ENOENT), "Scope has no PIDs. Refusing.");
  137
  138          return 0;
  139  }
```

### A failed load leaves the unit in an error state

`src/core/unit.c (v255)`, lines 1730-1748 (sha256 `d3c14e765dceeafd8e2d1fe94ed685c5dfc1c35f0a32c8459046a0459a1c57c4`):

```c
 1730  fail:
 1731          /* We convert ENOEXEC errors to the UNIT_BAD_SETTING load state here. Configuration parsing code
 1732           * should hence return ENOEXEC to ensure units are placed in this state after loading. */
 1733
 1734          u->load_state = u->load_state == UNIT_STUB ? UNIT_NOT_FOUND :
 1735                                       r == -ENOEXEC ? UNIT_BAD_SETTING :
 1736                                                       UNIT_ERROR;
 1737          u->load_error = r;
 1738
 1739          /* Record the timestamp on the cache, so that if the cache gets updated between now and the next time
 1740           * an attempt is made to load this unit, we know we need to check again. */
 1741          if (u->load_state == UNIT_NOT_FOUND)
 1742                  u->fragment_not_found_timestamp_hash = u->manager->unit_cache_timestamp_hash;
 1743
 1744          unit_add_to_dbus_queue(u);
 1745          unit_add_to_gc_queue(u);
 1746
 1747          return log_unit_debug_errno(u, r, "Failed to load configuration: %m");
 1748  }
```

### A start job is refused for a unit that did not load

`src/core/transaction.c (v255; identical in v255.4)`, lines 958-987 (sha256 `0d42c8e0372a31d94044b4c5ac30c2f0e80b35a53d27313a94f5109d11139f9c`):

```c
  958                  log_trace("Pulling in %s/%s from %s/%s", unit->id, job_type_to_string(type), by->unit->id, job_type_to_string(by->type));
  959
  960          /* Safety check that the unit is a valid state, i.e. not in UNIT_STUB or UNIT_MERGED which should only be set
  961           * temporarily. */
  962          if (!UNIT_IS_LOAD_COMPLETE(unit->load_state))
  963                  return sd_bus_error_setf(e, BUS_ERROR_LOAD_FAILED, "Unit %s is not loaded properly.", unit->id);
  964
  965          if (type != JOB_STOP) {
  966                  r = bus_unit_validate_load_state(unit, e);
  967                  /* The time-based cache allows to start new units without daemon-reload, but if they are
  968                   * already referenced (because of dependencies or ordering) then we have to force a load of
  969                   * the fragment. As an optimization, check first if anything in the usual paths was modified
  970                   * since the last time the cache was loaded. Also check if the last time an attempt to load
  971                   * the unit was made was before the most recent cache refresh, so that we know we need to try
  972                   * again — even if the cache is current, it might have been updated in a different context
  973                   * before we had a chance to retry loading this particular unit.
  974                   *
  975                   * Given building up the transaction is a synchronous operation, attempt
  976                   * to load the unit immediately. */
  977                  if (r < 0 && manager_unit_cache_should_retry_load(unit)) {
  978                          sd_bus_error_free(e);
  979                          unit->load_state = UNIT_STUB;
  980                          r = unit_load(unit);
  981                          if (r < 0 || unit->load_state == UNIT_STUB)
  982                                  unit->load_state = UNIT_NOT_FOUND;
  983                          r = bus_unit_validate_load_state(unit, e);
  984                  }
  985                  if (r < 0)
  986                          return r;
  987          }
```

### (the load-state check)

`src/core/dbus-unit.c (v255; identical in v255.4)`, lines 2528-2556 (sha256 `62445cc8de15136e2e7717d8cb5d497b8357d5476a0997ff9c66fc5ccce86725`):

```c
 2528  int bus_unit_validate_load_state(Unit *u, sd_bus_error *error) {
 2529          assert(u);
 2530
 2531          /* Generates a pretty error if a unit isn't properly loaded. */
 2532
 2533          switch (u->load_state) {
 2534
 2535          case UNIT_LOADED:
 2536                  return 0;
 2537
 2538          case UNIT_NOT_FOUND:
 2539                  return sd_bus_error_setf(error, BUS_ERROR_NO_SUCH_UNIT, "Unit %s not found.", u->id);
 2540
 2541          case UNIT_BAD_SETTING:
 2542                  return sd_bus_error_setf(error, BUS_ERROR_BAD_UNIT_SETTING, "Unit %s has a bad unit file setting.", u->id);
 2543
 2544          case UNIT_ERROR: /* Only show .load_error in UNIT_ERROR state */
 2545                  return sd_bus_error_set_errnof(error, u->load_error,
 2546                                                 "Unit %s failed to load properly, please adjust/correct and reload service manager: %m", u->id);
 2547
 2548          case UNIT_MASKED:
 2549                  return sd_bus_error_setf(error, BUS_ERROR_UNIT_MASKED, "Unit %s is masked.", u->id);
 2550
 2551          case UNIT_STUB:
 2552          case UNIT_MERGED:
 2553          default:
 2554                  return sd_bus_error_setf(error, BUS_ERROR_NO_SUCH_UNIT, "Unexpected load state of unit %s", u->id);
 2555          }
 2556  }
```

### Freeing a transient unit removes its transient files

`src/core/unit.c (v255)`, lines 747-760 (sha256 `d3c14e765dceeafd8e2d1fe94ed685c5dfc1c35f0a32c8459046a0459a1c57c4`):

```c
  747  Unit* unit_free(Unit *u) {
  748          Unit *slice;
  749          char *t;
  750
  751          if (!u)
  752                  return NULL;
  753
  754          sd_event_source_disable_unref(u->auto_start_stop_event_source);
  755
  756          u->transient_file = safe_fclose(u->transient_file);
  757
  758          if (!MANAGER_IS_RELOADING(u->manager))
  759                  unit_remove_transient(u);
  760
```

### (unit_remove_transient)

`src/core/unit.c (v255)`, lines 665-690 (sha256 `d3c14e765dceeafd8e2d1fe94ed685c5dfc1c35f0a32c8459046a0459a1c57c4`):

```c
  665  static void unit_remove_transient(Unit *u) {
  666          assert(u);
  667
  668          if (!u->transient)
  669                  return;
  670
  671          if (u->fragment_path)
  672                  (void) unlink(u->fragment_path);
  673
  674          STRV_FOREACH(i, u->dropin_paths) {
  675                  _cleanup_free_ char *p = NULL, *pp = NULL;
  676
  677                  if (path_extract_directory(*i, &p) < 0) /* Get the drop-in directory from the drop-in file */
  678                          continue;
  679
  680                  if (path_extract_directory(p, &pp) < 0) /* Get the config directory from the drop-in directory */
  681                          continue;
  682
  683                  /* Only drop transient drop-ins */
  684                  if (!path_equal(u->manager->lookup_paths.transient, pp))
  685                          continue;
  686
  687                  (void) unlink(*i);
  688                  (void) rmdir(p);
  689          }
  690  }
```

### zbus 4.4.0: the session and system builders read the environment; the address builder does not

`zbus-4.4.0/src/connection/builder.rs (crates.io, Cargo.lock checksum bb97012beadd29e654708a0fdb4c84bc046f537aecfde2c3ee0a9e4b4d48c725)`, lines 79-130 (sha256 `c8ba5bd025a13e9d4661552082dd89f15fdcdcf15d1c8a714cd6bb9e97e7c08b`):

```rust
   79  impl<'a> Builder<'a> {
   80      /// Create a builder for the session/user message bus connection.
   81      pub fn session() -> Result<Self> {
   82          Ok(Self::new(Target::Address(Address::session()?)))
   83      }
   84
   85      /// Create a builder for the system-wide message bus connection.
   86      pub fn system() -> Result<Self> {
   87          Ok(Self::new(Target::Address(Address::system()?)))
   88      }
   89
   90      /// Create a builder for connection that will use the given [D-Bus bus address].
   91      ///
   92      /// # Example
   93      ///
   94      /// Here is an example of connecting to an IBus service:
   95      ///
   96      /// ```no_run
   97      /// # use std::error::Error;
   98      /// # use zbus::connection::Builder;
   99      /// # use zbus::block_on;
  100      /// #
  101      /// # block_on(async {
  102      /// let addr = "unix:\
  103      ///     path=/home/zeenix/.cache/ibus/dbus-ET0Xzrk9,\
  104      ///     guid=fdd08e811a6c7ebe1fef0d9e647230da";
  105      /// let conn = Builder::address(addr)?
  106      ///     .build()
  107      ///     .await?;
  108      ///
  109      /// // Do something useful with `conn`..
  110      /// #     drop(conn);
  111      /// #     Ok::<(), zbus::Error>(())
  112      /// # }).unwrap();
  113      /// #
  114      /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
  115      /// ```
  116      ///
  117      /// **Note:** The IBus address is different for each session. You can find the address for your
  118      /// current session using `ibus address` command.
  119      ///
  120      /// [D-Bus bus address]: https://dbus.freedesktop.org/doc/dbus-specification.html#addresses
  121      pub fn address<A>(address: A) -> Result<Self>
  122      where
  123          A: TryInto<Address>,
  124          A::Error: Into<Error>,
  125      {
  126          Ok(Self::new(Target::Address(
  127              address.try_into().map_err(Into::into)?,
  128          )))
  129      }
  130
```

### (Address::session, Address::system)

`zbus-4.4.0/src/address/mod.rs`, lines 61-104 (sha256 `96924ae3dd0cd5ab0d482dc7cfd4b44ad24882d63f3cd780895005a4f256018a`):

```rust
   61
   62      /// Get the address for session socket respecting the DBUS_SESSION_BUS_ADDRESS environment
   63      /// variable. If we don't recognize the value (or it's not set) we fall back to
   64      /// $XDG_RUNTIME_DIR/bus
   65      pub fn session() -> Result<Self> {
   66          match env::var("DBUS_SESSION_BUS_ADDRESS") {
   67              Ok(val) => Self::from_str(&val),
   68              _ => {
   69                  #[cfg(windows)]
   70                  return Self::from_str("autolaunch:");
   71
   72                  #[cfg(all(unix, not(target_os = "macos")))]
   73                  {
   74                      let runtime_dir = env::var("XDG_RUNTIME_DIR")
   75                          .unwrap_or_else(|_| format!("/run/user/{}", Uid::effective()));
   76                      let path = format!("unix:path={runtime_dir}/bus");
   77
   78                      Self::from_str(&path)
   79                  }
   80
   81                  #[cfg(target_os = "macos")]
   82                  return Self::from_str("launchd:env=DBUS_LAUNCHD_SESSION_BUS_SOCKET");
   83              }
   84          }
   85      }
   86
   87      /// Get the address for system bus respecting the DBUS_SYSTEM_BUS_ADDRESS environment
   88      /// variable. If we don't recognize the value (or it's not set) we fall back to
   89      /// /var/run/dbus/system_bus_socket
   90      pub fn system() -> Result<Self> {
   91          match env::var("DBUS_SYSTEM_BUS_ADDRESS") {
   92              Ok(val) => Self::from_str(&val),
   93              _ => {
   94                  #[cfg(all(unix, not(target_os = "macos")))]
   95                  return Self::from_str("unix:path=/var/run/dbus/system_bus_socket");
   96
   97                  #[cfg(windows)]
   98                  return Self::from_str("autolaunch:");
   99
  100                  #[cfg(target_os = "macos")]
  101                  return Self::from_str("launchd:env=DBUS_LAUNCHD_SESSION_BUS_SOCKET");
  102              }
  103          }
  104      }
```

### Rust 1.94.0: set_var's safety contract

`library/std/src/env.rs (rust-lang/rust tag 1.94.0)`, lines 300-358 (sha256 `24031d1032a749d5d649ddc77a1abc7648b7229c1ef7b58612e621b6f82e0816`):

```rust
  300
  301  /// Sets the environment variable `key` to the value `value` for the currently running
  302  /// process.
  303  ///
  304  /// # Safety
  305  ///
  306  /// This function is safe to call in a single-threaded program.
  307  ///
  308  /// This function is also always safe to call on Windows, in single-threaded
  309  /// and multi-threaded programs.
  310  ///
  311  /// In multi-threaded programs on other operating systems, the only safe option is
  312  /// to not use `set_var` or `remove_var` at all.
  313  ///
  314  /// The exact requirement is: you
  315  /// must ensure that there are no other threads concurrently writing or
  316  /// *reading*(!) the environment through functions or global variables other
  317  /// than the ones in this module. The problem is that these operating systems
  318  /// do not provide a thread-safe way to read the environment, and most C
  319  /// libraries, including libc itself, do not advertise which functions read
  320  /// from the environment. Even functions from the Rust standard library may
  321  /// read the environment without going through this module, e.g. for DNS
  322  /// lookups from [`std::net::ToSocketAddrs`]. No stable guarantee is made about
  323  /// which functions may read from the environment in future versions of a
  324  /// library. All this makes it not practically possible for you to guarantee
  325  /// that no other thread will read the environment, so the only safe option is
  326  /// to not use `set_var` or `remove_var` in multi-threaded programs at all.
  327  ///
  328  /// Discussion of this unsafety on Unix may be found in:
  329  ///
  330  ///  - [Austin Group Bugzilla (for POSIX)](https://austingroupbugs.net/view.php?id=188)
  331  ///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
  332  ///
  333  /// To pass an environment variable to a child process, you can instead use [`Command::env`].
  334  ///
  335  /// [`std::net::ToSocketAddrs`]: crate::net::ToSocketAddrs
  336  /// [`Command::env`]: crate::process::Command::env
  337  ///
  338  /// # Panics
  339  ///
  340  /// This function may panic if `key` is empty, contains an ASCII equals sign `'='`
  341  /// or the NUL character `'\0'`, or when `value` contains the NUL character.
  342  ///
  343  /// # Examples
  344  ///
  345  /// ```
  346  /// use std::env;
  347  ///
  348  /// let key = "KEY";
  349  /// unsafe {
  350  ///     env::set_var(key, "VALUE");
  351  /// }
  352  /// assert_eq!(env::var(key), Ok("VALUE".to_string()));
  353  /// ```
  354  #[rustc_deprecated_safe_2024(
  355      audit_that = "the environment access only happens in single-threaded code"
  356  )]
  357  #[stable(feature = "env", since = "1.0.0")]
  358  pub unsafe fn set_var<K: AsRef<OsStr>, V: AsRef<OsStr>>(key: K, value: V) {
```
