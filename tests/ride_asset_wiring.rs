use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Every screen module, read once.
///
/// The walk is the point. This gate used to name `garden_screen.rs` as the file
/// that mounts the game, and broke the day that screen was deleted (D5) — not
/// because the property it guards stopped holding, but because it had hard-coded
/// where to look. A gate that names a file tests the filename.
fn screen_sources() -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    let screens = fs::read_dir("src/ui/screens").expect("read src/ui/screens");

    for entry in screens {
        let path = entry.expect("read screen entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = read(&path);
        sources.push((path, source));
    }

    assert!(
        sources.len() >= 20,
        "found only {} screen modules — a scan with an empty corpus passes by \
         default, and this repository has far more than that",
        sources.len()
    );
    sources
}

fn dynamic_game_imports() -> Vec<(PathBuf, String)> {
    let mut imports = Vec::new();

    for (path, source) in screen_sources() {
        for line in source.lines() {
            let Some(start) = line.find("import('/assets/game/") else {
                continue;
            };
            let import = &line[start + "import('".len()..];
            let end = import
                .find("')")
                .unwrap_or_else(|| panic!("unterminated dynamic import in {}", path.display()));
            imports.push((path.clone(), import[..end].to_string()));
        }
    }

    imports
}

fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start {start:?}"));
    let tail = &source[start..];
    let end = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing section end {end:?}"));
    &tail[..end]
}

fn fleet_bodies() -> BTreeSet<String> {
    let seed: serde_json::Value =
        serde_json::from_str(&read("data/fleet_seed.json")).expect("parse fleet seed");
    let families = seed
        .get("families")
        .and_then(serde_json::Value::as_array)
        .expect("fleet seed families array");
    assert!(!families.is_empty(), "fleet body gate found zero families");

    families
        .iter()
        .map(|family| {
            family
                .get("body")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("family has no string body: {family}"))
                .to_string()
        })
        .collect()
}

fn run_ride_model_in_node(assertions: &str) {
    let ride = read("assets/game/ride.js");
    let import_start = ride.find("import {").expect("ride engine import start");
    let import_marker = "} from '/assets/game/engine.js';";
    let import_end = import_start
        + ride[import_start..]
            .find(import_marker)
            .expect("ride engine import end")
        + import_marker.len();

    // Pure roster/handling tests do not need WebGL. Keep the production module
    // intact except for replacing its browser-only engine import with the two
    // top-level values these cases actually evaluate. createStage deliberately
    // throws so the startup-error path is executable without a GL context.
    let script = format!(
        "{}\nconst NEON = 0x39ff14;\nfunction createStage() {{ throw new Error('engine unavailable'); }}\n{}\n{}",
        &ride[..import_start],
        &ride[import_end..],
        assertions,
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("run Node.js Ride model regression");
    assert!(
        output.status.success(),
        "Ride model regression failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn live_ride_wiring_references_only_existing_game_assets() {
    let imports = dynamic_game_imports();
    assert!(
        !imports.is_empty(),
        "the wiring gate found zero dynamic game imports"
    );

    for (screen, public_path) in &imports {
        let repository_path = public_path.trim_start_matches('/');
        assert!(
            Path::new(repository_path).is_file(),
            "{} imports missing game asset {repository_path}",
            screen.display()
        );
    }

    assert!(
        imports
            .iter()
            .any(|(_, path)| path == "/assets/game/ride.js"),
        "no live screen imports /assets/game/ride.js"
    );
    assert!(
        imports
            .iter()
            .all(|(_, path)| path != "/assets/game/skate.js"),
        "the removed skate.js asset is still live"
    );

    let routes = read("src/ui/routes.rs");
    assert!(
        routes.contains("#[route(\"/ride\")]"),
        "the /ride route is absent"
    );
    assert!(
        routes.contains("#[route(\"/skate\")]"),
        "the historical /skate compatibility route is absent"
    );
    assert!(
        routes.contains("RideScreen {}"),
        "the route table never renders RideScreen"
    );

    let screens = read("src/ui/screens/mod.rs");
    assert!(
        screens.contains("pub mod ride_screen;"),
        "ride_screen is not declared"
    );
    assert!(
        screens.contains("pub use ride_screen::RideScreen;"),
        "RideScreen is not exported"
    );

    let sources = screen_sources();
    assert!(
        sources
            .iter()
            .any(|(_, source)| source.contains("RideGame {}")),
        "no screen mounts RideGame — the component exists and nothing renders it"
    );
    let skate: Vec<String> = sources
        .iter()
        .filter(|(_, source)| source.contains("SkateGame"))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        skate.is_empty(),
        "the retired SkateGame is still mounted by: {}",
        skate.join(", ")
    );
}

#[test]
fn ride_handling_and_silhouettes_keep_separate_catalog_inputs() {
    let ride = read("assets/game/ride.js");
    let handling = between(&ride, "// == Handling ==", "// == Silhouettes ==");

    assert!(handling.contains("const SPEED_INTERCEPT_GU = 400;"));
    assert!(handling.contains("const SPEED_PER_CC_GU = 1;"));
    assert!(handling.contains("const STEER_RATE_SU = { scooter: 120, motorcycle: 90 };"));
    assert!(handling.contains("const topSpeedGu = SPEED_INTERCEPT_GU + SPEED_PER_CC_GU * cc;"));
    assert!(handling.contains("const steerRateSu = STEER_RATE_SU[klass];"));
    assert!(handling
        .contains("if (!Object.prototype.hasOwnProperty.call(STEER_RATE_SU, klass)) return null;"));
    assert!(
        !handling.contains("family.body"),
        "body must select a silhouette without trimming canonical handling"
    );

    let silhouettes = between(&ride, "// == Silhouettes ==", "/// The name on screen.");
    for body in fleet_bodies() {
        assert!(
            silhouettes.contains(&format!("'{body}': {{")),
            "the catalog body {body} lost its explicit silhouette"
        );
    }
    assert!(
        silhouettes.contains("const SHAPE_BY_CLASS = { scooter: 'scooter', motorcycle: 'naked' };")
    );
}

#[test]
fn ride_hud_does_not_claim_real_world_speed_units() {
    let screen = read("src/ui/screens/ride_screen.rs");
    let ride = read("assets/game/ride.js");

    assert!(screen.contains("{distance} м · {speed} GU"));
    assert!(!screen.contains("km/h"));
    assert!(!ride.contains("km/h"));
    assert!(
        ride.contains("speed: Math.round(speed / GAME_SPEED_TO_WORLD)"),
        "the HUD must convert physics world speed back to the declared game units"
    );
}

#[test]
fn rideability_requires_explicit_positive_units_available() {
    let ride = read("assets/game/ride.js");
    let roster = between(
        &ride,
        "export function rideableFamilies",
        "/// Fetch the roster.",
    );

    assert!(
        roster.contains("if (family.offered !== true) continue;"),
        "a closed or malformed offer flag must not create a playable bike"
    );
    assert!(
        roster.contains("if (!Number.isInteger(units) || units < 1) continue;"),
        "missing, fractional, or non-positive availability must not create a playable bike"
    );
    assert!(
        roster.contains("units: Math.floor(units),"),
        "traffic capacity must come only from explicit catalog availability"
    );
    assert!(
        !roster.contains(": 1,"),
        "missing units_available still falls back to an invented unit"
    );
}

#[test]
fn ride_model_executes_handling_silhouette_and_offer_contract() {
    run_ride_model_in_node(
        r#"
const base = {
  key: 'seed-test', brand: 'Honda', model: 'NMAX', variant_label: '',
  class: 'scooter', body: 'sport', displacement_cc: 155,
  units_available: 1, offered: true,
};
const sport = handlingFor(base);
const cruiser = handlingFor({ ...base, body: 'cruiser' });
if (sport.topSpeedGu !== 555 || sport.steerRateSu !== 120) {
  throw new Error('canonical scooter handling did not execute');
}
if (JSON.stringify(sport) !== JSON.stringify(cruiser)) {
  throw new Error('body changed canonical handling');
}
if (shapeNameFor(base) !== 'sport' || shapeNameFor({ ...base, body: 'cruiser' }) !== 'cruiser') {
  throw new Error('catalog body did not select its silhouette');
}
if (handlingFor({ ...base, class: 'unknown' }) !== null) {
  throw new Error('unknown class became playable');
}
const candidates = [
  null,
  'malformed-row',
  base,
  { ...base, key: 'closed', offered: false },
  { ...base, key: 'missing-offered', offered: undefined },
  { ...base, key: 'missing-units', units_available: undefined },
  { ...base, key: 'zero-units', units_available: 0 },
];
const keys = rideableFamilies(candidates).map((entry) => entry.family.key);
if (JSON.stringify(keys) !== JSON.stringify(['seed-test'])) {
  throw new Error('rideability admitted a closed or unavailable family: ' + JSON.stringify(keys));
}
"#,
    );
}

#[test]
fn roster_failures_execute_without_an_end_event() {
    run_ride_model_in_node(
        r#"
console.warn = function () {};
globalThis.cancelAnimationFrame = function () {};
function fakeElement() {
  return {
    style: { cssText: '' }, parentNode: null, firstChild: null, children: [],
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
      this.firstChild = this.children[0] || null;
    },
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      this.firstChild = this.children[0] || null;
      child.parentNode = null;
    },
  };
}
globalThis.document = { createElement: function () { return fakeElement(); } };

async function observe(fetchImpl, withRosterCallback) {
  globalThis.fetch = fetchImpl;
  const ended = [];
  const roster = [];
  const opts = { onEnd: function (value) { ended.push(value); } };
  if (withRosterCallback) opts.onRoster = function (value) { roster.push(value); };
  const stop = start(fakeElement(), opts);
  await new Promise((resolve) => setTimeout(resolve, 0));
  stop();
  return { ended, roster };
}

const cases = [
  ['fetch', function () { return Promise.resolve({ ok: false, status: 503 }); }],
  ['fetch', function () { return Promise.resolve({ ok: true, json: function () { return Promise.resolve({}); } }); }],
  ['fetch', function () { return Promise.reject(new Error('offline')); }],
  ['empty', function () { return Promise.resolve({ ok: true, json: function () { return Promise.resolve({ bikes: [] }); } }); }],
  ['engine', function () { return Promise.resolve({ ok: true, json: function () { return Promise.resolve({ bikes: [{ key: 'nmax-155', brand: 'Yamaha', model: 'NMAX', class: 'scooter', body: 'scooter', displacement_cc: 155, units_available: 1, offered: true }] }); } }); }],
];
for (const [reason, fetchImpl] of cases) {
  const observed = await observe(fetchImpl, true);
  if (observed.ended.length !== 0) throw new Error(reason + ' emitted a real end');
  if (observed.roster.length !== 1 || observed.roster[0].reason !== reason) {
    throw new Error(reason + ' did not report its unavailable reason');
  }
  const legacyHost = await observe(fetchImpl, false);
  if (legacyHost.ended.length !== 0) throw new Error(reason + ' ended a host without onRoster');
}
"#,
    );
}

#[test]
fn host_routes_failures_away_from_the_only_submit_path() {
    let screen = read("src/ui/screens/ride_screen.rs");
    let ride = read("assets/game/ride.js");
    let unavailable = between(
        &ride,
        "function unavailable",
        "\n\n  setNotice(strings.loading);",
    );
    let finish = between(&ride, "function finish()", "\n\n    // Thrown up");
    let boot = between(
        &screen,
        "let js = r#\"(function(){",
        "\"#\n                .replace",
    );
    let rust_failure = between(
        &screen,
        "\"unavailable\" => {",
        "\n                    _ => {}",
    );

    assert!(!unavailable.contains("onEnd("));
    assert!(unavailable.contains("onRoster({ count: 0, reason"));
    assert_eq!(
        ride.matches("onEnd({").count(),
        1,
        "only actual finish may emit the real end callback"
    );
    assert!(finish.contains("onEnd({ distance: Math.floor(travelled), stars"));
    assert!(
        !finish.contains("travelled >") && !finish.contains("travelled === 0"),
        "a legitimate zero-distance ride completion must remain submit-eligible"
    );

    assert!(boot.contains("onRoster: function(s){"));
    assert!(boot.contains("{detail: {kind:'unavailable', reason:s.reason}}"));
    assert_eq!(boot.matches("kind:'end'").count(), 1);
    assert_eq!(boot.matches("kind:'unavailable'").count(), 2);
    let import_failure = between(&boot, ").catch(function(e){", "\n                });");
    assert!(import_failure.contains("kind:'unavailable', reason:'import'"));
    assert!(!import_failure.contains("kind:'end'"));

    assert!(!rust_failure.contains("submit.call("));
    assert_eq!(
        screen.matches("submit.call(").count(),
        1,
        "failure handling introduced a second submit path"
    );
}

#[test]
fn ride_end_captures_and_submits_each_run_once() {
    let screen = read("src/ui/screens/ride_screen.rs");
    let end = between(&screen, "\"end\" => {", "\n                    _ => {}");

    assert!(end.contains("let final_distance = get_number(\"distance\");"));
    assert!(end.contains("let final_stars = get_number(\"stars\");"));
    assert!(end.contains("if !submitted() {"));
    assert!(end.contains("submitted.set(true);"));
    assert!(end.contains("submit.call((final_distance, final_stars));"));
    assert!(
        !end.contains("final_distance >")
            && !end.contains("final_distance == 0")
            && !end.contains("final_stars >"),
        "actual completion must be classified by kind, not by a positive score threshold"
    );
    assert_eq!(
        screen.matches("submit.call(").count(),
        1,
        "an Again click or duplicate path can resubmit the previous run"
    );

    let capture = end
        .find("let final_distance")
        .expect("end handler captures its final distance");
    let submit = end
        .find("submit.call((final_distance, final_stars))")
        .expect("end handler submits its captured result");
    assert!(
        capture < submit,
        "submission must capture final values before another run can boot"
    );
}

#[test]
fn ride_dynamic_import_cannot_survive_unmount_or_a_new_boot() {
    let screen = read("src/ui/screens/ride_screen.rs");
    let boot = between(
        &screen,
        "let js = r#\"(function(){",
        "\"#\n                .replace",
    );

    assert!(screen.contains("use_hook_with_cleanup("));
    assert!(
        !screen.contains("listener.forget()"),
        "remounting Ride must not retain the previous component's event listener"
    );
    assert!(boot.contains("var generation = (window.__rideGeneration || 0) + 1;"));
    assert!(boot.contains("window.__rideGeneration = generation;"));
    assert!(
        boot.contains("if (window.__rideGeneration !== generation || !host.isConnected) return;")
    );
    assert!(boot.contains("var stop = m.start(host, {"));
    assert!(boot.contains("window.__rideStop = stop;"));

    let guard = boot
        .find("window.__rideGeneration !== generation")
        .expect("dynamic import continuation has a generation guard");
    let start = boot
        .find("var stop = m.start(host")
        .expect("dynamic import continuation mounts the game");
    assert!(
        guard < start,
        "a stale import reaches m.start before its guard"
    );

    let drop_cleanup = between(&screen, "use_drop(move || {", "\n    });\n\n    let title");
    assert!(drop_cleanup.contains("window.__rideGeneration=(window.__rideGeneration||0)+1;"));
    assert!(drop_cleanup.contains("window.__rideStop=null;"));
}

/// #13's second criterion, which nothing covered: "the mapping has a test
/// pinning at least three families."
///
/// `ride_model_executes_handling_silhouette_and_offer_contract` above executes
/// `handlingFor`, but against one synthetic 155cc row. That proves the function
/// runs; it does not pin the mapping to the fleet. Every family here is read
/// out of `data/fleet_seed.json` at test time rather than retyped, so this is
/// not a fourth copy of the catalog — if the seed's displacement or class for a
/// named family changes, this test computes the new expectation and the
/// assertion below still has to hold.
#[test]
fn ride_handling_is_pinned_against_real_families_from_the_seed() {
    let seed: serde_json::Value =
        serde_json::from_str(&read("data/fleet_seed.json")).expect("parse data/fleet_seed.json");
    let families = seed["families"]
        .as_array()
        .expect("the seed publishes a families array");
    assert!(
        families.len() >= 13,
        "only {} families in the seed — a scan with a near-empty corpus passes by default",
        families.len()
    );

    // The two #13 names by hand, plus one from the other class so the steering
    // lookup is exercised at both of its values. Three is the brief's floor.
    const PINNED: [&str; 4] = ["click-125", "xadv-750", "nmax-155", "cbr-650r"];

    let mut rows = String::new();
    for key in PINNED {
        let family = families
            .iter()
            .find(|f| f["key"].as_str() == Some(key))
            .unwrap_or_else(|| panic!("{key} is named by #13 but absent from the seed"));
        let class = family["class"].as_str().expect("class");
        let cc = family["displacement_cc"].as_i64().expect("displacement_cc");
        let body = family["body"].as_str().expect("body");

        // The expectation is recomputed from the seed, not stored: these are
        // the two constants `ride_handling_and_silhouettes_keep_separate_catalog_inputs`
        // already pins against the source.
        let top_speed_gu = 400 + cc;
        let steer_rate_su = match class {
            "scooter" => 120,
            "motorcycle" => 90,
            other => panic!("{key} has class {other:?}, which the steering lookup cannot answer"),
        };

        rows.push_str(&format!(
            "  {{ key: '{key}', class: '{class}', body: '{body}', displacement_cc: {cc}, \
             offered: true, units_available: 1, expectTopSpeedGu: {top_speed_gu}, \
             expectSteerRateSu: {steer_rate_su} }},\n"
        ));
    }

    run_ride_model_in_node(&format!(
        r#"
const PINNED = [
{rows}];

for (const row of PINNED) {{
  const h = handlingFor(row);
  if (h === null) {{
    throw new Error(row.key + ': a seeded family is not playable');
  }}
  if (h.topSpeedGu !== row.expectTopSpeedGu) {{
    throw new Error(row.key + ': topSpeedGu ' + h.topSpeedGu + ' != ' + row.expectTopSpeedGu);
  }}
  if (h.steerRateSu !== row.expectSteerRateSu) {{
    throw new Error(row.key + ': steerRateSu ' + h.steerRateSu + ' != ' + row.expectSteerRateSu);
  }}
}}

const byKey = Object.fromEntries(PINNED.map((row) => [row.key, handlingFor(row)]));

// #13's first criterion, in the exact two families it names. Top speed is
// affine in displacement, so 125cc and 750cc are 525 and 1150 game units —
// a factor of 2.2, not a rounding difference.
const click = byKey['click-125'];
const xadv = byKey['xadv-750'];
if (!(xadv.topSpeedGu > click.topSpeedGu * 2)) {{
  throw new Error('CLICK 125 and X-ADV 750 do not measurably differ in top speed');
}}

// And the half of that criterion the implementation does NOT meet, asserted
// as it actually behaves rather than left to be discovered: steering is a
// closed two-class lookup, and BOTH of these families are scooters in the
// seed. They steer identically, by design — displacement does not enter the
// steering rate at all. If the fleet ever reclassifies one of them, this
// assertion fails and #13's first criterion becomes fully true; either way
// nobody is left believing something the code does not do.
if (click.steerRateSu !== xadv.steerRateSu) {{
  throw new Error(
    'steering now differs between two same-class families — the lookup grew a ' +
    'displacement term, or the seed reclassified one of them'
  );
}}

// The other class is genuinely different, which is what makes the lookup
// worth having.
if (byKey['cbr-650r'].steerRateSu === click.steerRateSu) {{
  throw new Error('a motorcycle steers like a scooter — the class lookup is not being read');
}}
"#
    ));
}
