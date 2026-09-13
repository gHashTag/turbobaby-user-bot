// TurboBaby Ride - an endless low-poly ride on the bikes the shop really has.
//
// This module replaces assets/game/skate.js. DECISIONS.md D5: TurboBaby rents
// motorbikes, so the mini-game is a ride, and every machine on screen is a
// machine on the floor.
//
// The roster is fetched from GET /api/bikes?available_only=true when the game
// starts, and there is deliberately no built-in bike to fall back on: if that
// call fails, or comes back empty, the game says the roster is unavailable and
// offers no riders. A bike the shop cannot rent must not be rideable - the
// game is a shop window as much as it is a toy, and a machine nobody can hire
// is a promise the counter cannot keep.
//
// Same module contract as the other games here. It is a separate ES module so
// three.js (~650KB) is fetched only when a game mounts instead of riding along
// in the wasm bundle every customer downloads; it owns nothing outside its
// container; and start() returns a stop() that cancels the frame loop, drops
// the GL context and removes every listener, because a Mini App screen is
// opened and left many times in one session.
//
// ASCII only, on purpose. Player-visible copy comes in through opts.strings so
// the Rust side can supply ru/en out of the shared i18n table; the only other
// text on screen is a family's own name, straight from the catalog.

import {
  THREE, createStage, addLights, skyTexture, flat, buildWoody,
  contactShadow, Dust, horizontalInput, haptic, rand, pick,
  NEON,
} from '/assets/game/engine.js';

// == Roster ==
// Relative on purpose. The Mini App is served from the same origin as the API
// (src/ui/api/context.rs resolves the base URL to the page's own origin), so a
// relative path cannot be pointed at some other shop's stock.
const ROSTER_URL = '/api/bikes?available_only=true';

// English fallbacks only. The host passes opts.strings with the player's
// language; these exist so the module is never silent if it does not.
const DEFAULT_STRINGS = {
  loading: 'Loading the bikes in stock...',
  rosterUnavailable: 'The bike list is unavailable, so there is nothing to ride yet.',
  rosterEmpty: 'No bike is available to ride right now.',
  riding: 'Riding',
};

// == Palette ==
// The road and the sky are the skate game's, because it is the same island and
// two games on one stack should not look like two products.
const SKY_TOP = '#2f8fd8';
const SKY_MID = '#8ec9ea';
const SKY_LOW = '#cfeef7';
const HORIZON = 0xcfeef7;      // fog matches the bottom of the sky gradient
const ROAD = 0x44444a;
const LINE = 0xf2c94c;
const KERB = 0xe8d9a8;
const GRASS = 0x86c95f;
const FOLIAGE = [0x3f9142, 0x4faa4b, 0x2f7d3a];
const TRUNK = 0x8a5a3b;
const CONE = 0xf2711c;
const TYRE = 0x1b1b20;
const RIM = 0xb9bfc7;
const SEAT = 0x22222a;
const GLASS = 0x9fd8f0;
const LAMP = 0xfff3bd;

// Liveries are decoration, and only that. The colours TurboBaby has recorded
// are per physical unit (the catalog serves them on the family endpoint's unit
// rollup), not per family, so this hash deliberately does not pretend to be
// them - it exists so two bikes on the road are telling apart at a glance.
const LIVERY = [NEON, 0x2f63d8, 0xf24b3a, 0xf2a327, 0x36c9c0, 0xb46be0, 0xf6f8ff];

// == Feel ==
// Everything below is shared by every machine. What a machine does *not* share
// - how fast it runs, how hard it pulls, how quickly it turns - comes out of
// its own catalog row instead. See HANDLING.
const ROAD_HALF_WIDTH = 5;
const SEGMENT = 40;             // length of one recycled scenery slab
const SEGMENTS = 14;            // how far ahead the world exists
const TREES_PER_SLAB = 14;
const LANES = [-4, -2, 0, 2, 4];
// One obstacle row per twenty units. At speed that is a row every 0.4s, about
// as tight as analogue steering can answer; denser and the road ahead reads as
// a solid wall rather than as something to thread.
const BANDS = 2;
const DUST = 36;
const FOV = 62;
const SPIN = 1.9;               // wheel revolutions per unit travelled, near enough

// Other riders. Three is a road with traffic on it; more is a car park, and it
// costs a draw call per part.
const TRAFFIC_SLOTS = 3;
// A rider out on the road is not wringing its neck, so traffic cruises at a
// fraction of what its own machine can do. The player on a 750 therefore
// overtakes a 125 and not the other way round.
const TRAFFIC_PACE = 0.78;
const TRAFFIC_GAP = 46;         // minimum spacing between two traffic bikes
const RESPAWN_AHEAD_MIN = 170;
const RESPAWN_AHEAD_MAX = 260;
const RESPAWN_BEHIND_MIN = 60;
const RESPAWN_BEHIND_MAX = 120;
const DESPAWN_BEHIND = 44;      // past the camera
const DESPAWN_AHEAD = 330;      // beyond the fog, so it can be moved unseen

// == Handling ==
// The canonical game-unit map is specs/turbobaby/ride_game.t27: top speed is
// affine in displacement and steering is a closed two-class lookup. Body still
// selects a truthful silhouette below, but it never changes physics.
const SPEED_INTERCEPT_GU = 400;
const SPEED_PER_CC_GU = 1;
const STEER_RATE_SU = { scooter: 120, motorcycle: 90 };

// Physics consumes world units per second, not the contract's game units.
// These are CHOSEN global conversions: every family passes through the same
// constants, so no hidden per-family or per-body tuning can drift from the
// canonical map. Acceleration derives only from converted top speed and lean
// only from the canonical steer rate.
const GAME_SPEED_TO_WORLD = 0.055;
const GAME_STEER_TO_WORLD = 0.09;
const ACCEL_PER_WORLD_TOP_SPEED = 0.03;
const LEAN_PER_STEER_SU = 0.0025;
const LAUNCH_FRACTION = 0.55;

/// Everything a family rides like, derived from its own row.
///
/// Returns null when the row cannot support a ride: a family with no usable
/// displacement or an unknown class would have to be handed an invented
/// number, and inventing a number for a bike is no more acceptable here than
/// it is for a price.
export function handlingFor(family) {
  if (!family || typeof family !== 'object') return null;
  const cc = Number(family.displacement_cc);
  if (!Number.isInteger(cc) || cc <= 0 || cc > 65535) return null;

  const klass = String(family.class || '');
  if (!Object.prototype.hasOwnProperty.call(STEER_RATE_SU, klass)) return null;

  const topSpeedGu = SPEED_INTERCEPT_GU + SPEED_PER_CC_GU * cc;
  const steerRateSu = STEER_RATE_SU[klass];
  const topSpeed = topSpeedGu * GAME_SPEED_TO_WORLD;

  return {
    cc,
    class: klass,
    topSpeedGu,
    steerRateSu,
    topSpeed,
    launchSpeed: topSpeed * LAUNCH_FRACTION,
    accel: topSpeed * ACCEL_PER_WORLD_TOP_SPEED,
    steer: steerRateSu * GAME_STEER_TO_WORLD,
    lean: steerRateSu * LEAN_PER_STEER_SU,
  };
}

// == Silhouettes ==
// One entry per body the catalog uses, so no family in the fleet falls through
// to a placeholder box: scooter, maxi-scooter, adventure-scooter, naked, sport
// and cruiser cover all fourteen rows in data/fleet_seed.json.
//
// The numbers are proportions, not specifications. Nothing here claims to be a
// measurement of a real machine - they are what makes a step-through read as a
// step-through and a cruiser read as a cruiser from six metres back.
const SHAPES = {
  'scooter': {
    name: 'scooter', wheel: 0.30, tyre: 0.13, wheelbase: 1.90, deck: true,
    tankH: 0, seatY: 0.64, seatZ: 0.34, barW: 0.58, barY: 0.95,
    cowlH: 0.70, cowlD: 0.34, screenH: 0, rake: 0.10, pegY: 0.24, pitch: -0.04,
  },
  'maxi-scooter': {
    name: 'maxi-scooter', wheel: 0.32, tyre: 0.14, wheelbase: 2.20, deck: true,
    tankH: 0, seatY: 0.70, seatZ: 0.40, barW: 0.62, barY: 1.02,
    cowlH: 0.95, cowlD: 0.42, screenH: 0.46, rake: 0.12, pegY: 0.26, pitch: -0.02,
  },
  'adventure-scooter': {
    name: 'adventure-scooter', wheel: 0.38, tyre: 0.15, wheelbase: 2.25, deck: true,
    tankH: 0, seatY: 0.80, seatZ: 0.42, barW: 0.70, barY: 1.14,
    cowlH: 0.86, cowlD: 0.38, screenH: 0.40, rake: 0.16, pegY: 0.32, pitch: -0.06,
  },
  'naked': {
    name: 'naked', wheel: 0.34, tyre: 0.14, wheelbase: 2.05, deck: false,
    tankH: 0.70, seatY: 0.72, seatZ: 0.38, barW: 0.66, barY: 1.00,
    cowlH: 0.30, cowlD: 0.22, screenH: 0, rake: 0.13, pegY: 0.34, pitch: -0.10,
  },
  'sport': {
    name: 'sport', wheel: 0.34, tyre: 0.14, wheelbase: 2.05, deck: false,
    tankH: 0.68, seatY: 0.76, seatZ: 0.44, barW: 0.54, barY: 0.92,
    cowlH: 0.78, cowlD: 0.40, screenH: 0.30, rake: 0.11, pegY: 0.36, pitch: -0.22,
  },
  'cruiser': {
    name: 'cruiser', wheel: 0.36, tyre: 0.16, wheelbase: 2.45, deck: false,
    tankH: 0.72, seatY: 0.58, seatZ: 0.48, barW: 0.76, barY: 1.02,
    cowlH: 0.26, cowlD: 0.20, screenH: 0, rake: 0.24, pegY: 0.30, pitch: 0.08,
  },
};
// A body the catalog gains later still gets a real silhouette rather than a
// cube: the shop's own class decides which of the two families of shape it
// belongs to. The bike is still a real bike - only its outline generalises.
const SHAPE_BY_CLASS = { scooter: 'scooter', motorcycle: 'naked' };

/// Which silhouette a family is drawn with.
export function shapeNameFor(family) {
  const body = String((family && family.body) || '');
  if (Object.prototype.hasOwnProperty.call(SHAPES, body)) return body;
  const klass = String((family && family.class) || '');
  if (Object.prototype.hasOwnProperty.call(SHAPE_BY_CLASS, klass)) return SHAPE_BY_CLASS[klass];
  return 'naked';
}

/// The name on screen. `variant_label` is part of it where the shop sells two
/// tariffs under one model name ("XMAX 300 NEW 2023+"): dropping it would put
/// two different products on screen under one label.
export function labelFor(family) {
  const parts = [];
  for (const field of ['brand', 'model', 'variant_label']) {
    const value = family && family[field];
    if (typeof value === 'string' && value.length) parts.push(value);
  }
  return parts.join(' ');
}

function liveryFor(key) {
  const s = String(key || '');
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 100000;
  return LIVERY[h % LIVERY.length];
}

/// The families that may appear on screen, each with its handling resolved.
///
/// Three independent reasons to drop a row, none of them a guess: the family
/// is closed to new rentals, it has no unit free today, or its catalog fields
/// cannot produce canonical handling. The endpoint already filters on offer
/// and availability, but a caller may point `rosterUrl` elsewhere.
export function rideableFamilies(list) {
  const out = [];
  if (!Array.isArray(list)) return out;
  for (const family of list) {
    const handling = handlingFor(family);
    if (!handling) continue;
    if (family.offered !== true) continue;
    const units = Number(family.units_available);
    // Availability is part of the rideable contract, not an optional hint.
    // Missing or malformed data must not turn into an invented spare bike.
    if (!Number.isInteger(units) || units < 1) continue;
    out.push({
      family,
      handling,
      shape: SHAPES[shapeNameFor(family)],
      label: labelFor(family),
      livery: liveryFor(family.key),
      units: Math.floor(units),
    });
  }
  return out;
}

/// Fetch the roster. Resolves to `{ error, families }` and never rejects, so
/// every failure path ends at the same honest empty state.
function loadRoster(url, abort) {
  const init = { headers: { accept: 'application/json' } };
  if (abort) init.signal = abort.signal;
  return fetch(url, init).then(function (res) {
    if (!res.ok) return { error: 'http_' + res.status, families: [] };
    return res.json().then(function (body) {
      if (!body || !Array.isArray(body.bikes)) return { error: 'payload', families: [] };
      return { error: null, families: body.bikes };
    });
  }).catch(function (e) {
    // Leaving the screen mid-flight aborts the request, which is not a fault
    // and must not be logged as one.
    if (abort && abort.signal.aborted) return { error: 'aborted', families: [] };
    console.warn('ride: roster unavailable', e);
    return { error: 'network', families: [] };
  });
}

/// Release every GPU buffer the run allocated. `renderer.dispose()` drops the
/// context but not the geometries and materials hanging off the scene graph,
/// and this game builds a bike per rider rather than one fixed set of props.
function disposeTree(root) {
  root.traverse(function (node) {
    if (node.geometry) node.geometry.dispose();
    const m = node.material;
    if (Array.isArray(m)) {
      for (const one of m) if (one) one.dispose();
    } else if (m) {
      m.dispose();
    }
  });
}

/// How many of each family may be on the road at once.
///
/// Bounded by `units_available`: the shop has one X-ADV, so the road gets one
/// X-ADV, and the one the player is riding is that one. Round-robin so a full
/// fleet shows variety before it shows a second of anything.
function trafficPlan(roster, playerIndex, limit) {
  const budget = roster.map(function (entry, i) {
    return i === playerIndex ? entry.units - 1 : entry.units;
  });
  const out = [];
  let progress = true;
  while (out.length < limit && progress) {
    progress = false;
    for (let i = 0; i < roster.length && out.length < limit; i++) {
      if (budget[i] <= 0) continue;
      budget[i] -= 1;
      out.push(roster[i]);
      progress = true;
    }
  }
  return out;
}

export function start(container, opts = {}) {
  const onStar = opts.onStar || function () { };
  const onEnd = opts.onEnd || function () { };
  const onTick = opts.onTick || function () { };
  // Optional, and new in this game: the host may want to say which bike is
  // being ridden, or why there is nothing to ride. A host that does not pass
  // it still gets a correct screen.
  const onRoster = opts.onRoster || null;
  const strings = Object.assign({}, DEFAULT_STRINGS, opts.strings || {});
  const rosterUrl = opts.rosterUrl || ROSTER_URL;

  let stopped = false;
  let raf = 0;
  let stage = null;
  let detachInput = null;
  let notice = null;
  let plate = null;
  let sim = null;
  const abort = typeof AbortController === 'undefined' ? null : new AbortController();

  // == Overlays ==
  // Two DOM layers rather than in-scene text: a font in three.js costs another
  // download, and this copy has to be readable before the GL context exists -
  // the roster message is shown when there will be no scene at all.
  function overlay(style) {
    const el = document.createElement('div');
    el.style.cssText = style;
    container.appendChild(el);
    return el;
  }

  function setNotice(text) {
    if (!notice) {
      notice = overlay('position:absolute;inset:0;z-index:7;display:flex;'
        + 'align-items:flex-start;justify-content:center;padding:14% 22px 0;'
        + 'pointer-events:none;font:600 14px/1.5 system-ui,sans-serif;');
      const pill = document.createElement('div');
      pill.style.cssText = 'max-width:300px;text-align:center;color:#fff;'
        + 'background:rgba(8,12,18,0.78);border:2px solid rgba(57,255,20,0.45);'
        + 'padding:12px 16px;';
      notice.appendChild(pill);
    }
    // textContent, not innerHTML: the roster is server text and the family
    // names in it are data, never markup.
    notice.firstChild.textContent = text;
  }

  function clearNotice() {
    if (notice && notice.parentNode) notice.parentNode.removeChild(notice);
    notice = null;
  }

  function setPlate(text) {
    if (!plate) {
      plate = overlay('position:absolute;left:14px;bottom:12px;z-index:4;'
        + 'pointer-events:none;color:#fff;font:700 12px/1.4 system-ui,sans-serif;'
        + 'text-shadow:1px 1px 0 rgba(0,0,0,0.45);background:rgba(0,0,0,0.28);'
        + 'padding:5px 10px;max-width:70%;');
    }
    plate.textContent = text;
  }

  function stop() {
    stopped = true;
    if (sim) sim.alive = false;
    cancelAnimationFrame(raf);
    if (abort) abort.abort();
    if (detachInput) {
      detachInput();
      detachInput = null;
    }
    clearNotice();
    if (plate && plate.parentNode) plate.parentNode.removeChild(plate);
    plate = null;
    if (stage) {
      disposeTree(stage.scene);
      stage.dispose();
      stage = null;
    }
    sim = null;
  }

  // No roster, no riders. The message is the whole screen: there is no
  // stand-in bike to put on it, because a bike that is not in stock has no
  // business being ridden in the shop's own game.
  function unavailable(reason, text) {
    setNotice(text);
    if (onRoster) onRoster({ count: 0, reason, riding: null, key: null });
  }

  setNotice(strings.loading);
  loadRoster(rosterUrl, abort).then(function (result) {
    if (stopped) return;               // the screen was left while we fetched
    if (result.error) {
      unavailable('fetch', strings.rosterUnavailable);
      return;
    }
    const roster = rideableFamilies(result.families);
    if (!roster.length) {
      unavailable('empty', strings.rosterEmpty);
      return;
    }
    clearNotice();
    try {
      begin(roster);
    } catch (e) {
      console.error('ride: could not start', e);
      unavailable('engine', strings.rosterUnavailable);
    }
  });

  function begin(roster) {
    // The shop's own order comes first (sort_order, then displacement), so
    // index 0 is the bike the shop puts in front of a walk-in. A host that
    // wants a picker passes the chosen key.
    const wanted = String(opts.familyKey || '');
    let playerIndex = 0;
    for (let i = 0; i < roster.length; i++) {
      if (roster[i].family.key === wanted) {
        playerIndex = i;
        break;
      }
    }
    const player = roster[playerIndex];
    const handling = player.handling;

    stage = createStage(container, { fov: FOV, far: 500 });
    const { renderer, scene, camera } = stage;

    scene.background = skyTexture(SKY_TOP, SKY_MID, SKY_LOW);
    // Fog hides the point where the recycled world ends, so the road reads as
    // endless rather than as a strip that stops. Its colour has to be the
    // sky's horizon colour or the seam comes back as a band.
    scene.fog = new THREE.Fog(HORIZON, 70, 260);
    addLights(scene);

    // == Shared geometry and materials ==
    // Keyed, so a second bike of the same body and a fourteenth road slab cost
    // draw calls but not memory. Everything here is released by disposeTree().
    const cache = new Map();
    function cached(key, make) {
      let value = cache.get(key);
      if (value === undefined) {
        value = make();
        cache.set(key, value);
      }
      return value;
    }
    function cachedMat(color) {
      return cached('mat:' + color, function () { return flat(color); });
    }
    function box(key, w, h, d, color) {
      const geo = cached('box:' + key, function () { return new THREE.BoxGeometry(w, h, d); });
      return new THREE.Mesh(geo, cachedMat(color));
    }

    // == Sky furniture ==
    // Unfogged and far away, so the horizon has something in it, parented to a
    // group that trails the camera slowly - which reads as parallax.
    const sky = new THREE.Group();
    scene.add(sky);
    {
      // Unlit and flattened: a lit icosahedron up there reads as a floating
      // boulder, not a cloud, because half its faces fall into shadow.
      const cloudGeo = new THREE.IcosahedronGeometry(1, 0);
      for (let i = 0; i < 8; i++) {
        const cloud = new THREE.Group();
        const puffs = 3 + ((Math.random() * 3) | 0);
        for (let j = 0; j < puffs; j++) {
          const puff = new THREE.Mesh(cloudGeo, new THREE.MeshBasicMaterial({
            color: j % 2 ? 0xffffff : 0xeef6ff, fog: false,
          }));
          const r = rand(7, 13);
          puff.position.set(rand(-14, 14), rand(-1.5, 1.5), rand(-2, 2));
          puff.scale.set(r, r * rand(0.4, 0.6), r * 0.6);
          cloud.add(puff);
        }
        cloud.position.set(rand(-220, 220), rand(52, 105), rand(-380, -250));
        sky.add(cloud);
      }
      const disc = new THREE.Mesh(
        new THREE.CircleGeometry(15, 16),
        new THREE.MeshBasicMaterial({ color: LAMP, fog: false }));
      disc.position.set(-120, 104, -350);
      sky.add(disc);
    }

    // == World ==
    const world = new THREE.Group();
    scene.add(world);

    const trunkGeo = new THREE.CylinderGeometry(0.16, 0.28, 1, 5);
    const crownGeo = new THREE.IcosahedronGeometry(1, 0);
    const trunkMat = cachedMat(TRUNK);
    const crownMat = new THREE.MeshLambertMaterial({ flatShading: true });
    // Big and strongly emissive: a collectible the player cannot pick out from
    // the road markings at a hundred metres is not a collectible.
    const starGeo = new THREE.OctahedronGeometry(0.7, 0);
    const starMat = new THREE.MeshLambertMaterial({
      color: 0xffd54a, emissive: 0xffae00, emissiveIntensity: 0.75, flatShading: true,
    });
    const haloGeo = new THREE.RingGeometry(0.85, 1.15, 12);
    const haloMat = new THREE.MeshBasicMaterial({
      color: 0xfff0a0, transparent: true, opacity: 0.45, side: THREE.DoubleSide, depthWrite: false,
    });
    // Roadworks cones rather than boulders: this is a road, and a cone is what
    // is actually lying about on it.
    const coneGeo = new THREE.ConeGeometry(0.42, 1.05, 8);
    const coneMat = cachedMat(CONE);
    const groundGeo = new THREE.PlaneGeometry(180, SEGMENT);
    const roadGeo = new THREE.PlaneGeometry(ROAD_HALF_WIDTH * 2, SEGMENT);
    const lineGeo = new THREE.PlaneGeometry(0.35, SEGMENT);
    const dashGeo = new THREE.PlaneGeometry(0.22, SEGMENT * 0.14);
    const kerbGeo = new THREE.BoxGeometry(0.5, 0.28, SEGMENT);
    const grassMat = cachedMat(GRASS);
    const roadMat = cachedMat(ROAD);
    const lineMat = cachedMat(LINE);
    const kerbMat = cachedMat(KERB);

    const dummy = new THREE.Object3D();
    const tint = new THREE.Color();

    const slabs = [];
    for (let i = 0; i < SEGMENTS; i++) {
      const slab = new THREE.Group();

      const ground = new THREE.Mesh(groundGeo, grassMat);
      ground.rotation.x = -Math.PI / 2;
      slab.add(ground);

      const road = new THREE.Mesh(roadGeo, roadMat);
      road.rotation.x = -Math.PI / 2;
      road.position.y = 0.01;
      slab.add(road);

      for (const side of [-1, 1]) {
        const line = new THREE.Mesh(lineGeo, lineMat);
        line.rotation.x = -Math.PI / 2;
        line.position.set(side * (ROAD_HALF_WIDTH - 0.3), 0.02, 0);
        slab.add(line);

        const kerb = new THREE.Mesh(kerbGeo, kerbMat);
        kerb.position.set(side * (ROAD_HALF_WIDTH + 0.25), 0.14, 0);
        slab.add(kerb);
      }

      // A dashed centre line: the strongest cue that the world is moving, and
      // it costs three quads.
      for (let d = -1; d <= 1; d++) {
        const dash = new THREE.Mesh(dashGeo, lineMat);
        dash.rotation.x = -Math.PI / 2;
        dash.position.set(0, 0.02, d * (SEGMENT / 3));
        slab.add(dash);
      }

      // Fourteen slabs of loose tree meshes is several hundred draw calls,
      // which is what makes phones stutter; two instanced meshes is two.
      const trunks = new THREE.InstancedMesh(trunkGeo, trunkMat, TREES_PER_SLAB);
      const crowns = new THREE.InstancedMesh(crownGeo, crownMat, TREES_PER_SLAB);
      trunks.frustumCulled = false;
      crowns.frustumCulled = false;
      slab.add(trunks, crowns);
      slab.userData.trunks = trunks;
      slab.userData.crowns = crowns;

      slab.userData.props = [];
      world.add(slab);
      slabs.push(slab);
    }

    function dressSlab(slab, clear) {
      for (const p of slab.userData.props) slab.remove(p);
      slab.userData.props = [];

      const { trunks, crowns } = slab.userData;
      for (let i = 0; i < TREES_PER_SLAB; i++) {
        const side = Math.random() < 0.5 ? -1 : 1;
        const h = rand(3.4, 8.5);
        const x = side * rand(ROAD_HALF_WIDTH + 2.5, ROAD_HALF_WIDTH + 40);
        const z = rand(-SEGMENT / 2, SEGMENT / 2);

        dummy.position.set(x, h * 0.35, z);
        dummy.rotation.set(0, rand(0, Math.PI), 0);
        dummy.scale.set(1, h * 0.7, 1);
        dummy.updateMatrix();
        trunks.setMatrixAt(i, dummy.matrix);

        const r = rand(1.5, 2.8);
        dummy.position.set(x, h * 0.7 + r * 0.5, z);
        dummy.rotation.set(rand(0, 1), rand(0, Math.PI), rand(0, 1));
        dummy.scale.set(r, r * rand(0.7, 1.1), r);
        dummy.updateMatrix();
        crowns.setMatrixAt(i, dummy.matrix);
        crowns.setColorAt(i, tint.setHex(pick(FOLIAGE)));
      }
      trunks.instanceMatrix.needsUpdate = true;
      crowns.instanceMatrix.needsUpdate = true;
      if (crowns.instanceColor) crowns.instanceColor.needsUpdate = true;

      if (clear) return;   // the slabs under the rider at 0 m carry nothing

      // Cones come in rows, and a row never takes more than two of the five
      // lanes - with a traffic bike possibly holding a third, two lanes are
      // always open. An endless runner that can generate an unavoidable wall
      // is not a game.
      for (let b = 0; b < BANDS; b++) {
        const z = -SEGMENT / 2 + (b + 0.5) * (SEGMENT / BANDS) + rand(-2, 2);
        const free = LANES.slice();
        const cones = Math.random() < 0.25 ? 2 : 1;
        for (let c = 0; c < cones; c++) {
          const lane = free.splice((Math.random() * free.length) | 0, 1)[0];
          const cone = new THREE.Mesh(coneGeo, coneMat);
          cone.position.set(lane + rand(-0.3, 0.3), 0.52, z);
          cone.rotation.y = rand(0, 3);
          cone.userData.kind = 'cone';
          slab.add(cone);
          slab.userData.props.push(cone);
        }
        if (Math.random() < 0.8) {
          const lane = free.splice((Math.random() * free.length) | 0, 1)[0];
          const star = new THREE.Mesh(starGeo, starMat);
          star.position.set(lane, 1.35, z);
          star.userData.kind = 'star';
          star.userData.pop = 0;
          // A halo facing the camera, so the star is a bright disc even when
          // the octahedron happens to be edge-on.
          const halo = new THREE.Mesh(haloGeo, haloMat);
          star.add(halo);
          star.userData.halo = halo;
          slab.add(star);
          slab.userData.props.push(star);
        }
      }
    }

    slabs.forEach(function (slab, i) {
      slab.position.z = -i * SEGMENT;
      // Three clear slabs: the first cone then stands about 110 m out, which is
      // enough road to read before anything has to be dodged.
      dressSlab(slab, i <= 2);
    });

    // == Machines ==
    // Built from primitives, for the same reason three.js is lazy-loaded: no
    // extra megabytes on a phone in a beach bar. Each one is assembled from
    // its family's silhouette, so a step-through and a cruiser are different
    // objects on screen and not one object with two names.
    function buildBike(entry) {
      const s = entry.shape;
      const n = s.name;
      const bike = new THREE.Group();
      const wheels = [];
      const half = s.wheelbase / 2;

      const tyreGeo = cached('tyre:' + n, function () {
        return new THREE.CylinderGeometry(s.wheel, s.wheel, s.tyre, 12);
      });
      const rimGeo = cached('rim:' + n, function () {
        return new THREE.CylinderGeometry(s.wheel * 0.45, s.wheel * 0.45, s.tyre + 0.04, 8);
      });
      for (const z of [-half, half]) {
        const tyre = new THREE.Mesh(tyreGeo, cachedMat(TYRE));
        tyre.rotation.z = Math.PI / 2;
        tyre.position.set(0, s.wheel, z);
        const rim = new THREE.Mesh(rimGeo, cachedMat(RIM));
        rim.rotation.z = Math.PI / 2;
        rim.position.set(0, s.wheel, z);
        bike.add(tyre, rim);
        wheels.push(tyre, rim);
      }

      const spine = box('spine:' + n, 0.18, 0.14, s.wheelbase * 0.86, entry.livery);
      spine.position.set(0, s.wheel + 0.2, 0);
      bike.add(spine);

      if (s.deck) {
        // A step-through's floorboard, which is most of what makes a scooter
        // look like a scooter from behind.
        const deck = box('deck:' + n, 0.62, 0.09, 0.95, entry.livery);
        deck.position.set(0, s.pegY, 0.28);
        bike.add(deck);
      } else {
        const tank = box('tank:' + n, 0.44, 0.30, 0.68, entry.livery);
        tank.position.set(0, s.tankH - 0.15, -0.10);
        bike.add(tank);
      }

      const saddle = box('seat:' + n, 0.42, 0.12, 0.62, SEAT);
      saddle.position.set(0, s.seatY - 0.06, s.seatZ);
      bike.add(saddle);

      const tail = box('tail:' + n, 0.34, 0.17, 0.30, entry.livery);
      tail.position.set(0, s.seatY + 0.03, s.seatZ + 0.44);
      bike.add(tail);

      const cowl = box('cowl:' + n, 0.52, s.cowlH, s.cowlD, entry.livery);
      cowl.position.set(0, s.wheel + s.cowlH / 2 + 0.16, -half + 0.12);
      bike.add(cowl);

      if (s.screenH > 0) {
        const screen = new THREE.Mesh(
          cached('screen:' + n, function () {
            return new THREE.BoxGeometry(0.46, s.screenH, 0.04);
          }),
          cached('glass', function () {
            return new THREE.MeshLambertMaterial({
              color: GLASS, transparent: true, opacity: 0.55, flatShading: true,
            });
          }));
        screen.position.set(0, s.wheel + s.cowlH + s.screenH / 2 + 0.14, -half + 0.22);
        screen.rotation.x = -0.30;
        bike.add(screen);
      }

      const fork = box('fork:' + n, 0.30, s.barY - s.wheel, 0.09, RIM);
      fork.position.set(0, (s.barY + s.wheel) / 2, -half + 0.06);
      fork.rotation.x = s.rake;
      bike.add(fork);

      const bars = new THREE.Mesh(
        cached('bars:' + n, function () {
          return new THREE.CylinderGeometry(0.035, 0.035, s.barW, 6);
        }),
        cachedMat(TYRE));
      bars.rotation.z = Math.PI / 2;
      bars.position.set(0, s.barY, -half + 0.30);
      bike.add(bars);

      const lamp = new THREE.Mesh(
        cached('lamp:' + n, function () { return new THREE.BoxGeometry(0.26, 0.16, 0.08); }),
        cached('lampmat', function () { return new THREE.MeshBasicMaterial({ color: LAMP }); }));
      lamp.position.set(0, s.wheel + s.cowlH * 0.72, -half + 0.02);
      bike.add(lamp);

      bike.userData.wheels = wheels;
      return bike;
    }

    // == Player ==
    const rider = new THREE.Group();
    const bike = buildBike(player);
    rider.add(bike);

    // The mascot comes from the engine, so a change to Woody reaches every
    // game. He is not articulated at the hip, so he stands on the pegs rather
    // than sitting - at this scale, from behind, that is what a rider looks
    // like, and it keeps him the same bird as in the other two games.
    const { bird, headGroup, tail, armL, armR } = buildWoody({ headTurn: 0 });
    bird.scale.setScalar(0.82);
    bird.position.set(0, player.shape.pegY, player.shape.seatZ - 0.10);
    bird.rotation.x = player.shape.pitch;
    rider.add(bird);

    // A helmet, because the shop hands one over with every key. Open-face and
    // cut off above the brow so the crest still comes through it - the
    // silhouette is what has to say "Woody" at six metres.
    {
      const shell = new THREE.Mesh(
        new THREE.SphereGeometry(0.34, 10, 6, 0, Math.PI * 2, 0, Math.PI * 0.62),
        cachedMat(player.livery));
      shell.position.set(0, 0.05, 0.03);
      headGroup.add(shell);
      const visor = new THREE.Mesh(
        new THREE.BoxGeometry(0.30, 0.11, 0.08),
        cached('visor', function () {
          return new THREE.MeshLambertMaterial({
            color: 0x12161c, transparent: true, opacity: 0.8, flatShading: true,
          });
        }));
      visor.position.set(0, 0.04, -0.26);
      headGroup.add(visor);
    }
    scene.add(rider);

    // A fake contact shadow. Real shadow maps cost far more than this on a
    // phone and buy nothing here, but without something the bike looks like it
    // is hovering rather than rolling.
    const shadow = contactShadow(0.55, player.shape.wheelbase * 0.62);
    scene.add(shadow);

    // == Traffic ==
    const slots = [];
    const plan = trafficPlan(roster, playerIndex, TRAFFIC_SLOTS);
    for (let i = 0; i < plan.length; i++) {
      const entry = plan[i];
      const group = buildBike(entry);
      // Anonymous riders: Woody is one bird, and four of him on one road both
      // cheapens him and costs thirty draw calls.
      const torso = new THREE.Mesh(
        cached('npc-torso', function () { return new THREE.CapsuleGeometry(0.22, 0.34, 3, 6); }),
        cachedMat(0x3b4250));
      torso.position.set(0, entry.shape.seatY + 0.34, entry.shape.seatZ - 0.06);
      const head = new THREE.Mesh(
        cached('npc-head', function () { return new THREE.SphereGeometry(0.19, 8, 6); }),
        cachedMat(entry.livery));
      head.position.set(0, entry.shape.seatY + 0.78, entry.shape.seatZ - 0.10);
      group.add(torso, head);
      scene.add(group);
      slots.push({
        entry,
        group,
        lane: LANES[i % LANES.length],
        x: 0,
        z: -(70 + i * 80),
        // Deterministic: this rider's cruise is what its own machine can do,
        // scaled once. Nothing here is randomised, so a slow bike is slow
        // because it is a slow bike.
        speed: entry.handling.topSpeed * TRAFFIC_PACE,
        phase: rand(0, 6.2),
      });
    }

    function respawnSlot(slot, ahead) {
      for (let tries = 0; tries < 8; tries++) {
        const z = ahead
          ? -rand(RESPAWN_AHEAD_MIN, RESPAWN_AHEAD_MAX)
          : rand(RESPAWN_BEHIND_MIN, RESPAWN_BEHIND_MAX);
        let clear = true;
        for (const other of slots) {
          if (other !== slot && Math.abs(other.z - z) < TRAFFIC_GAP) {
            clear = false;
            break;
          }
        }
        if (!clear) continue;
        slot.z = z;
        slot.lane = pick(LANES);
        slot.phase = rand(0, 6.2);
        return;
      }
      // Eight tries and every stretch is busy: park it beyond the fog rather
      // than stack it on top of another rider.
      slot.z = -RESPAWN_AHEAD_MAX - rand(0, 60);
      slot.lane = pick(LANES);
    }

    // == Dust ==
    const dust = new Dust(scene, DUST);

    // == Input ==
    // Steering is a target the bike eases toward, so a touch that jumps across
    // the screen does not teleport it across the road.
    let steer = 0;                 // -1..1
    let steerShown = 0;            // eased, drives the lean
    detachInput = horizontalInput(renderer.domElement, function (v) { steer = v; });

    // == Loop ==
    let x = 0;
    let speed = handling.launchSpeed;
    let travelled = 0;
    let stars = 0;
    let clock = 0;
    let shake = 0;
    let crash = 0;
    let last = performance.now();

    setPlate(strings.riding + ' ' + player.label);
    if (onRoster) {
      onRoster({
        count: roster.length,
        reason: null,
        riding: player.label,
        key: player.family.key,
      });
    }

    sim = {
      alive: true,
      roster,
      player,
      slots,
      setSteer: function (v) { steer = v; },
      state: function () {
        return { x, speed, stars, distance: Math.floor(travelled) };
      },
    };

    function finish() {
      if (!sim || !sim.alive) return;
      sim.alive = false;
      shake = 1;
      haptic('heavy');
      onEnd({ distance: Math.floor(travelled), stars, bike: player.family.key });
    }

    // Thrown up behind the rear tyre, wherever this machine's rear tyre is.
    const dustZ = player.shape.wheelbase / 2 + 0.2;

    function spawnDust(dt) {
      // Roughly one mote per 25ms flat out, scaled by how fast we are going.
      if (Math.random() > dt * 40 * (speed / handling.topSpeed)) return;
      dust.spawn(x, 0.12, dustZ);
    }

    function frame(now) {
      raf = requestAnimationFrame(frame);
      // Clamped so a backgrounded tab does not resume with one enormous step
      // that teleports the bike through a cone.
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      clock += dt;
      const alive = sim ? sim.alive : false;

      if (alive) {
        speed = Math.min(handling.topSpeed, speed + handling.accel * dt);
        travelled += speed * dt;

        x += steer * handling.steer * dt;
        const limit = ROAD_HALF_WIDTH - 0.6;
        if (x < -limit) x = -limit;
        if (x > limit) x = limit;
      } else {
        // Coast to a stop after a crash rather than freezing the world dead.
        speed = Math.max(0, speed - 46 * dt);
        crash = Math.min(1, crash + dt * 2.2);
      }

      steerShown += (steer - steerShown) * Math.min(1, dt * 9);
      const span = handling.topSpeed - handling.launchSpeed;
      const pace = span > 0 ? Math.max(0, Math.min(1, (speed - handling.launchSpeed) / span)) : 0;

      // == Rider ==
      rider.position.x = x;
      // Lean derives from the canonical class steering rate through the one
      // shared conversion above; body changes silhouette, never physics.
      rider.rotation.z = -steerShown * handling.lean;
      rider.position.y = Math.sin(clock * 9) * 0.02 * (0.4 + pace);
      bird.rotation.z = steerShown * 0.1;                      // counter-lean
      bird.rotation.y = -steerShown * 0.18;
      // A woodpecker that never pecks is just a red bird.
      headGroup.rotation.x = Math.sin(clock * 11) * 0.06 + 0.03;
      headGroup.rotation.z = steerShown * 0.14;
      tail.rotation.y = Math.sin(clock * 5) * 0.12 - steerShown * 0.16;
      // Both hands stay on the bars, so the arms reach forward and only the
      // wrists follow the steering.
      armL.rotation.x = -1.15;
      armR.rotation.x = -1.15;
      armL.rotation.z = -0.55 - steerShown * 0.12;
      armR.rotation.z = 0.55 - steerShown * 0.12;
      for (const w of bike.userData.wheels) w.rotation.x -= speed * dt * SPIN;

      if (crash > 0) {
        rider.rotation.z += crash * 1.5;
        rider.position.y += Math.sin(crash * Math.PI) * 1.2;
        bird.rotation.x = player.shape.pitch + crash * 2.2;
      }

      shadow.position.set(x, 0.03, 0);
      shadow.material.opacity = 0.28 * (1 - crash);

      // == Camera ==
      // FOV widens with speed; that alone does most of the work of making 60
      // units/sec feel fast. The shake is only on the crash.
      camera.fov = FOV + pace * 7;
      camera.updateProjectionMatrix();
      const jitter = shake * 0.4;
      // High enough and far enough back that the rider sits in the lower third
      // and the road ahead - where the cones are - stays unobstructed.
      camera.position.set(
        x * 0.75 + steerShown * 0.5 + rand(-jitter, jitter),
        4.6 + rand(-jitter, jitter),
        9.2 + crash * 1.5);
      camera.lookAt(x * 0.35 + steerShown * 0.9, 1.2, -18);
      camera.rotation.z = -steerShown * 0.06;
      shake = Math.max(0, shake - dt * 2.5);

      sky.position.x = x * 0.15;

      // == World ==
      // Move the world toward the camera and recycle slabs behind it.
      for (const slab of slabs) {
        slab.position.z += speed * dt;
        if (slab.position.z > SEGMENT) {
          slab.position.z -= SEGMENTS * SEGMENT;
          dressSlab(slab, false);
        }
        for (const p of slab.userData.props) {
          if (!p.userData.kind) continue;
          if (p.userData.kind === 'star') {
            p.rotation.y += dt * 3;
            p.position.y = 1.35 + Math.sin(clock * 3 + p.position.z) * 0.12;
            // Undo the parent's spin so the halo keeps facing down the road.
            if (p.userData.halo) p.userData.halo.rotation.y = -p.rotation.y;
            if (p.userData.pop > 0) {
              // A collected star pops and rises instead of blinking out - the
              // reward has to be visible or it does not feel like one.
              p.userData.pop += dt * 3;
              p.position.y += p.userData.pop * 2.2;
              p.scale.setScalar(1 + p.userData.pop * 1.6);
              if (p.userData.pop > 1) {
                p.visible = false;
                p.userData.pop = 0;
                p.scale.setScalar(1);
              }
              continue;
            }
          }
          // World position along z; the rider sits at z = 0.
          const pz = slab.position.z + p.position.z;
          if (alive && pz > -1.4 && pz < 1.4 && Math.abs(p.position.x - x) < 1.0 && p.visible) {
            if (p.userData.kind === 'star') {
              p.userData.pop = 0.001;
              stars += 1;
              haptic('light');
              onStar(stars);
            } else {
              finish();
            }
          }
          if (pz < -SEGMENT * 2) {
            p.visible = true;
            p.userData.pop = 0;
            p.scale.setScalar(1);
          }
        }
      }

      // == Traffic ==
      for (const slot of slots) {
        // Relative motion, because the world moves and the player does not:
        // a rider slower than the player drifts back into view behind them.
        slot.z += (speed - slot.speed) * dt;
        if (slot.z > DESPAWN_BEHIND) respawnSlot(slot, true);
        else if (slot.z < -DESPAWN_AHEAD) respawnSlot(slot, false);

        slot.x = slot.lane + Math.sin(clock * 0.6 + slot.phase) * 0.18;
        slot.group.position.set(slot.x, 0, slot.z);
        slot.group.rotation.z = Math.sin(clock * 1.7 + slot.phase) * 0.05;
        for (const w of slot.group.userData.wheels) w.rotation.x -= slot.speed * dt * SPIN;

        if (alive && Math.abs(slot.z) < 1.7 && Math.abs(slot.x - x) < 0.95) finish();
      }

      // == Dust ==
      if (alive) spawnDust(dt);
      dust.update(dt, speed * 0.55);

      if (alive) {
        onTick({
          distance: Math.floor(travelled),
          stars,
          // The simulation moves in world units, but the HUD names the
          // canonical contract unit. Convert back with the same global scale
          // instead of relabelling a renderer quantity as GU.
          speed: Math.round(speed / GAME_SPEED_TO_WORLD),
        });
      }
      renderer.render(scene, camera);
    }
    raf = requestAnimationFrame(frame);
  }

  // A test seam, in the shape catch.js established. The point the harness
  // cannot otherwise reach is the roster: whether the bikes on screen are the
  // ones the endpoint served, and whether an empty answer really does leave no
  // rider at all. `handlingFor` is exported as well, so the mapping can be
  // pinned without a GL context.
  stop.debug = {
    rosterKeys: () => (sim ? sim.roster.map((e) => e.family.key) : []),
    ridingKey: () => (sim ? sim.player.family.key : null),
    trafficKeys: () => (sim ? sim.slots.map((s) => s.entry.family.key) : []),
    handling: () => (sim ? sim.player.handling : null),
    noticeText: () => (notice ? notice.firstChild.textContent : null),
    plateText: () => (plate ? plate.textContent : null),
    setSteer: (v) => { if (sim) sim.setSteer(Math.max(-1, Math.min(1, v))); },
    state: () => (sim ? sim.state() : null),
  };

  return stop;
}
