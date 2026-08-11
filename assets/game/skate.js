// Woody Skate — an endless low-poly downhill run starring the brand's woodpecker.
//
// Deliberately a separate ES module rather than part of the Rust/wasm bundle:
// three.js is ~650KB, and the shop's bundle is downloaded by every customer who
// opens the menu. Nobody buying tea should pay for a game they never open, so
// this and the library are fetched only when the game screen mounts.
//
// The module owns nothing outside its container and cleans up after itself —
// `start()` returns a `stop()` that cancels the frame loop, drops the GL
// context and removes its listeners, because a Mini App screen can be opened
// and left many times in one session.

import * as THREE from '/assets/vendor/three.module.min.js';

// ── Palette ─────────────────────────────────────────────────────────────────
// Taken off assets/logo.jpg so the rider in the game and the avatar in the bot
// are recognisably the same bird.
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

const CREST = 0xf24b3a;        // the bright red comb
const FEATHER = 0xe0342a;      // head and tail
const BEAK = 0xf2a327;
const HOODIE = 0x2f63d8;
const HOODIE_DARK = 0x1f47a8;
const TRIM = 0xf6f8ff;
const FOOT = 0xe8902a;
const DECK = 0x39ff14;         // the shop's neon green
const WHEEL = 0xf3e04a;

// ── Feel ────────────────────────────────────────────────────────────────────
// Tuned so a run lasts long enough to be worth starting and short enough to
// retry. Speed climbs slowly; the lane is wide enough to dodge on a phone.
const ROAD_HALF_WIDTH = 5;
const START_SPEED = 26;
const MAX_SPEED = 62;
const ACCEL = 0.9;              // units/sec added per second
const SEGMENT = 40;             // length of one recycled scenery slab
const SEGMENTS = 14;            // how far ahead the world exists
const STEER = 9;                // lateral units per second at full input
const TREES_PER_SLAB = 14;
const LANES = [-4, -2, 0, 2, 4];
// One obstacle row per twenty units. At top speed that is a row every 0.4s,
// which is about as tight as analogue steering can answer; denser than this
// and the road ahead reads as a solid wall of rock.
const BANDS = 2;                // obstacle rows per slab
const DUST = 36;

function rand(a, b) { return a + Math.random() * (b - a); }
function pick(arr) { return arr[(Math.random() * arr.length) | 0]; }

/// A vertical gradient used as the sky. Four pixels wide because only the
/// vertical axis carries any information.
function skyTexture() {
  const c = document.createElement('canvas');
  c.width = 4; c.height = 256;
  const g = c.getContext('2d');
  const grad = g.createLinearGradient(0, 0, 0, 256);
  // The pale stop has to land on the horizon, which sits a little above the
  // middle of the frame — otherwise the sky is still blue where the fog has
  // already gone white and the seam shows as a band.
  grad.addColorStop(0, SKY_TOP);
  grad.addColorStop(0.38, SKY_MID);
  grad.addColorStop(0.52, SKY_LOW);
  grad.addColorStop(1, SKY_LOW);
  g.fillStyle = grad;
  g.fillRect(0, 0, 4, 256);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

export function start(container, opts = {}) {
  const onStar = opts.onStar || function () { };
  const onEnd = opts.onEnd || function () { };
  const onTick = opts.onTick || function () { };

  // The container is often still unlaid-out on the frame the screen mounts, so
  // `clientWidth` reads 0 and the canvas is created 0x0 — the game then runs
  // perfectly and draws nothing. Measure with a fallback, and keep watching.
  function measure() {
    const w = container.clientWidth || container.parentElement?.clientWidth || window.innerWidth;
    const h = container.clientHeight || container.parentElement?.clientHeight || window.innerHeight;
    return { w: Math.max(1, w), h: Math.max(1, h) };
  }

  const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'low-power' });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  {
    const { w, h } = measure();
    renderer.setSize(w, h);
  }
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  scene.background = skyTexture();
  // Fog hides the point where the recycled world ends, so the road reads as
  // endless rather than as a strip that stops. Its colour has to be the sky's
  // horizon colour or the seam becomes visible again as a band.
  scene.fog = new THREE.Fog(HORIZON, 70, 260);

  const camera = new THREE.PerspectiveCamera(62, measure().w / measure().h, 0.1, 500);

  // Three lights, each doing one job: hemisphere for the ambient sky/ground
  // bounce, a warm key for shape, a dim cool fill so the shadowed sides do not
  // go flat black.
  scene.add(new THREE.HemisphereLight(0xd8f0ff, 0x6f8f5a, 1.0));
  const sun = new THREE.DirectionalLight(0xfff2d0, 0.85);
  sun.position.set(-8, 16, 6);
  scene.add(sun);
  const fill = new THREE.DirectionalLight(0x9fd0ff, 0.25);
  fill.position.set(7, 6, -8);
  scene.add(fill);

  const flat = (color) => new THREE.MeshLambertMaterial({ color, flatShading: true });
  const mesh = (geo, color) => new THREE.Mesh(geo, flat(color));

  // ── Sky furniture ────────────────────────────────────────────────────────
  // Unfogged and far away, so the horizon has something in it. Parented to a
  // group that trails the camera slowly, which reads as parallax.
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
      new THREE.MeshBasicMaterial({ color: 0xfff3bd, fog: false }));
    disc.position.set(-120, 104, -350);
    sky.add(disc);
  }

  // ── World ────────────────────────────────────────────────────────────────
  const world = new THREE.Group();
  scene.add(world);

  // Geometry and materials are created once and shared by every slab. Foliage
  // additionally goes through InstancedMesh: fourteen slabs of loose tree
  // meshes is several hundred draw calls, which is what makes phones stutter.
  const trunkGeo = new THREE.CylinderGeometry(0.16, 0.28, 1, 5);
  const crownGeo = new THREE.IcosahedronGeometry(1, 0);
  const trunkMat = new THREE.MeshLambertMaterial({ color: TRUNK, flatShading: true });
  const crownMat = new THREE.MeshLambertMaterial({ flatShading: true });
  // Big and strongly emissive: a collectible the player cannot pick out from
  // the road markings at a hundred metres is not a collectible.
  const starGeo = new THREE.OctahedronGeometry(0.7, 0);
  const starMat = new THREE.MeshLambertMaterial({ color: 0xffd54a, emissive: 0xffae00, emissiveIntensity: 0.75, flatShading: true });
  const haloGeo = new THREE.RingGeometry(0.85, 1.15, 12);
  const haloMat = new THREE.MeshBasicMaterial({
    color: 0xfff0a0, transparent: true, opacity: 0.45, side: THREE.DoubleSide, depthWrite: false,
  });
  const rockGeo = new THREE.DodecahedronGeometry(0.8, 0);
  const rockMat = flat(0x55555e);
  const groundGeo = new THREE.PlaneGeometry(180, SEGMENT);
  const roadGeo = new THREE.PlaneGeometry(ROAD_HALF_WIDTH * 2, SEGMENT);
  const lineGeo = new THREE.PlaneGeometry(0.35, SEGMENT);
  const dashGeo = new THREE.PlaneGeometry(0.22, SEGMENT * 0.14);
  const kerbGeo = new THREE.BoxGeometry(0.5, 0.28, SEGMENT);
  const grassMat = flat(GRASS);
  const roadMat = flat(ROAD);
  const lineMat = flat(LINE);
  const kerbMat = flat(KERB);

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

    // A dashed centre line: the strongest cue that the world is moving, and it
    // costs three quads.
    for (let d = -1; d <= 1; d++) {
      const dash = new THREE.Mesh(dashGeo, lineMat);
      dash.rotation.x = -Math.PI / 2;
      dash.position.set(0, 0.02, d * (SEGMENT / 3));
      slab.add(dash);
    }

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

    // Trees along both verges, written straight into the instance matrices.
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

    if (clear) return;   // the slabs under the rider at 0 m carry no obstacles

    // Obstacles are laid out in rows, and a row never fills every lane — an
    // endless runner that can generate an unavoidable wall is not a game.
    for (let b = 0; b < BANDS; b++) {
      const z = -SEGMENT / 2 + (b + 0.5) * (SEGMENT / BANDS) + rand(-2, 2);
      const free = LANES.slice();
      const rocks = Math.random() < 0.25 ? 2 : 1;
      for (let r = 0; r < rocks; r++) {
        const lane = free.splice((Math.random() * free.length) | 0, 1)[0];
        const rock = new THREE.Mesh(rockGeo, rockMat);
        rock.position.set(lane + rand(-0.3, 0.3), 0.55, z);
        rock.rotation.set(rand(0, 3), rand(0, 3), rand(0, 3));
        rock.scale.setScalar(rand(0.8, 1.15));
        rock.userData.kind = 'rock';
        slab.add(rock);
        slab.userData.props.push(rock);
      }
      if (Math.random() < 0.8) {
        const lane = free.splice((Math.random() * free.length) | 0, 1)[0];
        const star = new THREE.Mesh(starGeo, starMat);
        star.position.set(lane, 1.35, z);
        star.userData.kind = 'star';
        star.userData.pop = 0;
        // A halo facing the camera, so the star is a bright disc even when the
        // octahedron happens to be edge-on.
        const halo = new THREE.Mesh(haloGeo, haloMat);
        star.add(halo);
        star.userData.halo = halo;
        slab.add(star);
        slab.userData.props.push(star);
      }
    }
  }

  slabs.forEach((slab, i) => {
    slab.position.z = -i * SEGMENT;
    // Three clear slabs: the first rock then stands about 110 m out, roughly
    // four seconds at starting speed, which is enough to read the road before
    // having to dodge anything.
    dressSlab(slab, i <= 2);
  });

  // ── Rider: Woody, the brand's woodpecker ─────────────────────────────────
  // Built from primitives rather than loaded as a model, for the same reason
  // three.js is lazy-loaded: no extra megabytes on a phone in a beach bar.
  // The silhouette is what has to be recognisable at speed, so the swept-back
  // red crest and the long beak are oversized on purpose.
  const rider = new THREE.Group();

  const board = new THREE.Group();
  {
    const deck = mesh(new THREE.BoxGeometry(0.95, 0.09, 2.7), DECK);
    const grip = mesh(new THREE.BoxGeometry(0.86, 0.03, 2.5), 0x14141a);
    grip.position.y = 0.06;
    for (const [z, tilt] of [[-1.55, 0.42], [1.55, -0.42]]) {
      const kick = mesh(new THREE.BoxGeometry(0.95, 0.09, 0.6), DECK);
      kick.position.set(0, 0.07, z);
      kick.rotation.x = tilt;
      board.add(kick);
    }
    for (const zt of [-0.95, 0.95]) {
      const truck = mesh(new THREE.BoxGeometry(0.5, 0.14, 0.2), 0x9aa0a8);
      truck.position.set(0, -0.06, zt);
      board.add(truck);
    }
    const wheelGeo = new THREE.CylinderGeometry(0.17, 0.17, 0.16, 8);
    for (const sx of [-0.38, 0.38]) {
      for (const sz of [-0.95, 0.95]) {
        const wheel = mesh(wheelGeo, WHEEL);
        wheel.rotation.z = Math.PI / 2;
        wheel.position.set(sx, -0.11, sz);
        board.add(wheel);
        (board.userData.wheels ||= []).push(wheel);
      }
    }
    board.add(deck, grip);
    board.position.y = 0.3;
  }

  const bird = new THREE.Group();
  const headGroup = new THREE.Group();
  const tail = new THREE.Group();
  const armL = new THREE.Group();
  const armR = new THREE.Group();
  {
    for (const sx of [-0.2, 0.2]) {
      const leg = mesh(new THREE.CylinderGeometry(0.07, 0.07, 0.42, 6), FOOT);
      leg.position.set(sx, 0.26, 0);
      bird.add(leg);
      const foot = mesh(new THREE.BoxGeometry(0.3, 0.09, 0.52), FOOT);
      foot.position.set(sx, 0.06, 0);
      bird.add(foot);
    }

    const body = mesh(new THREE.CapsuleGeometry(0.42, 0.42, 3, 8), HOODIE);
    body.position.y = 0.95;
    bird.add(body);

    // Hood bunched at the back, and the white fur trim from the logo.
    const hood = mesh(new THREE.IcosahedronGeometry(0.3, 0), HOODIE_DARK);
    hood.position.set(0, 1.24, 0.3);
    bird.add(hood);
    const collar = mesh(new THREE.TorusGeometry(0.34, 0.11, 4, 10), TRIM);
    collar.rotation.x = Math.PI / 2;
    collar.position.y = 1.3;
    bird.add(collar);
    const zip = mesh(new THREE.BoxGeometry(0.07, 0.62, 0.04), TRIM);
    zip.position.set(0, 0.95, -0.41);
    bird.add(zip);

    // Arms out for balance; each in its own group so it can swing about the
    // shoulder rather than about its own centre.
    for (const [group, sx] of [[armL, -1], [armR, 1]]) {
      const upper = mesh(new THREE.BoxGeometry(0.17, 0.52, 0.17), HOODIE);
      upper.position.y = -0.26;
      const cuff = mesh(new THREE.BoxGeometry(0.19, 0.1, 0.19), TRIM);
      cuff.position.y = -0.5;
      const glove = mesh(new THREE.IcosahedronGeometry(0.16, 0), TRIM);
      glove.position.y = -0.62;
      group.add(upper, cuff, glove);
      group.position.set(sx * 0.45, 1.15, 0);
      group.rotation.z = sx * 0.95;
      bird.add(group);
    }

    // Tail feathers, fanned.
    for (let i = -1; i <= 1; i++) {
      const feather = mesh(new THREE.BoxGeometry(0.15, 0.06, 0.8), FEATHER);
      feather.position.set(i * 0.14, 0, 0.4);
      feather.rotation.set(-0.25, i * 0.22, 0);
      tail.add(feather);
    }
    tail.position.set(0, 0.78, 0.32);
    bird.add(tail);

    const neck = mesh(new THREE.CylinderGeometry(0.15, 0.19, 0.3, 7), FEATHER);
    neck.position.y = 1.44;
    bird.add(neck);

    const head = mesh(new THREE.IcosahedronGeometry(0.33, 0), FEATHER);
    headGroup.add(head);

    const upperBeak = mesh(new THREE.ConeGeometry(0.13, 0.66, 5), BEAK);
    upperBeak.rotation.x = -Math.PI / 2;
    upperBeak.position.set(0, 0.02, -0.5);
    const lowerBeak = mesh(new THREE.ConeGeometry(0.1, 0.5, 5), 0xd98a15);
    lowerBeak.rotation.x = -Math.PI / 2;
    lowerBeak.position.set(0, -0.12, -0.42);
    headGroup.add(upperBeak, lowerBeak);

    for (const sx of [-1, 1]) {
      const white = mesh(new THREE.IcosahedronGeometry(0.13, 0), 0xffffff);
      white.position.set(sx * 0.19, 0.1, -0.22);
      const pupil = mesh(new THREE.IcosahedronGeometry(0.06, 0), 0x11131a);
      pupil.position.set(sx * 0.22, 0.1, -0.31);
      headGroup.add(white, pupil);
    }

    // The comb: three cones sweeping back over the skull. This is the shape
    // people read as "Woody" from fifty metres away.
    const crestSizes = [[0.17, 0.62, 0.16, -0.9], [0.15, 0.52, 0.36, -1.15], [0.12, 0.4, 0.54, -1.35]];
    for (const [r, h, z, tilt] of crestSizes) {
      const spike = mesh(new THREE.ConeGeometry(r, h, 5), CREST);
      spike.position.set(0, 0.3, z);
      spike.rotation.x = tilt;
      headGroup.add(spike);
    }

    headGroup.position.y = 1.78;
    headGroup.rotation.y = -0.75;   // turned enough that the beak shows in profile
    bird.add(headGroup);

    bird.position.y = 0.42;
    bird.rotation.y = -0.3;         // skate stance, without hiding the crest
  }

  rider.add(board, bird);
  scene.add(rider);

  // A fake contact shadow. Real shadow maps cost far more than this on a phone
  // and buy nothing here, but without *something* the rider looks like it is
  // hovering rather than rolling.
  const shadow = new THREE.Mesh(
    new THREE.CircleGeometry(1, 16),
    new THREE.MeshBasicMaterial({ color: 0x0a1a08, transparent: true, opacity: 0.28, depthWrite: false }));
  shadow.rotation.x = -Math.PI / 2;
  shadow.scale.set(0.75, 1.5, 1);
  scene.add(shadow);

  // ── Dust ─────────────────────────────────────────────────────────────────
  // Kicked up behind the wheels. One instanced mesh, so one draw call, and it
  // does more for the sense of speed than any amount of extra scenery.
  const dust = new THREE.InstancedMesh(
    new THREE.IcosahedronGeometry(0.09, 0),
    // Unlit, or the motes turn into little grey pebbles bouncing off the road.
    new THREE.MeshBasicMaterial({ color: 0xf3f7ee, transparent: true, opacity: 0.4, depthWrite: false }),
    DUST);
  dust.frustumCulled = false;
  scene.add(dust);
  const motes = [];
  for (let i = 0; i < DUST; i++) motes.push({ x: 0, y: 0, z: 0, life: 0, vx: 0, vy: 0, scale: 1 });

  // ── Input ────────────────────────────────────────────────────────────────
  // Steering is a target the rider eases toward, so a touch that jumps across
  // the screen does not teleport the board.
  let steer = 0;                 // -1..1
  let steerShown = 0;            // eased, drives the visual lean
  let pointerActive = false;

  const setSteerFromX = (clientX) => {
    const rect = renderer.domElement.getBoundingClientRect();
    const t = (clientX - rect.left) / rect.width;      // 0..1
    steer = Math.max(-1, Math.min(1, (t - 0.5) * 2.4));
  };
  const onDown = (e) => { pointerActive = true; setSteerFromX(e.touches ? e.touches[0].clientX : e.clientX); };
  const onMove = (e) => { if (pointerActive) setSteerFromX(e.touches ? e.touches[0].clientX : e.clientX); };
  const onUp = () => { pointerActive = false; steer = 0; };
  const onKey = (e, down) => {
    if (e.key === 'ArrowLeft' || e.key === 'a') steer = down ? -1 : 0;
    if (e.key === 'ArrowRight' || e.key === 'd') steer = down ? 1 : 0;
  };
  const keyDown = (e) => onKey(e, true);
  const keyUp = (e) => onKey(e, false);

  const el = renderer.domElement;
  el.style.display = 'block';
  el.addEventListener('touchstart', onDown, { passive: true });
  el.addEventListener('touchmove', onMove, { passive: true });
  el.addEventListener('touchend', onUp, { passive: true });
  el.addEventListener('mousedown', onDown);
  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', onUp);
  window.addEventListener('keydown', keyDown);
  window.addEventListener('keyup', keyUp);

  const onResize = () => {
    const { w, h } = measure();
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
  };
  window.addEventListener('resize', onResize);
  // A window `resize` never fires when the element itself is laid out late,
  // which is exactly the case that produced the 0x0 canvas.
  let ro = null;
  if (typeof ResizeObserver !== 'undefined') {
    ro = new ResizeObserver(onResize);
    ro.observe(container);
  }
  requestAnimationFrame(onResize);

  // ── Loop ─────────────────────────────────────────────────────────────────
  let x = 0, speed = START_SPEED, travelled = 0, stars = 0, alive = true, raf = 0;
  let clock = 0, shake = 0, crash = 0;
  let last = performance.now();

  function finish() {
    if (!alive) return;
    alive = false;
    shake = 1;
    onEnd({ distance: Math.floor(travelled), stars });
  }

  function spawnDust(dt) {
    // Roughly one mote per 25ms at full speed, scaled by how fast we are going.
    if (Math.random() > dt * 40 * (speed / MAX_SPEED)) return;
    const m = motes[(Math.random() * DUST) | 0];
    m.x = x + rand(-0.4, 0.4);
    m.y = 0.15;
    m.z = 1.4;
    m.vx = rand(-1.2, 1.2);
    m.vy = rand(1.2, 3.0);
    m.life = 1;
    m.scale = rand(0.6, 1.4);
  }

  function frame(now) {
    raf = requestAnimationFrame(frame);
    // Clamped so a backgrounded tab does not resume with one enormous step
    // that teleports the rider through a rock.
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    clock += dt;

    if (alive) {
      speed = Math.min(MAX_SPEED, speed + ACCEL * dt);
      travelled += speed * dt;

      x += steer * STEER * dt;
      const limit = ROAD_HALF_WIDTH - 0.6;
      if (x < -limit) x = -limit;
      if (x > limit) x = limit;
    } else {
      // Coast to a stop after a crash rather than freezing the world dead.
      speed = Math.max(0, speed - 46 * dt);
      crash = Math.min(1, crash + dt * 2.2);
    }

    steerShown += (steer - steerShown) * Math.min(1, dt * 9);
    const pace = (speed - START_SPEED) / (MAX_SPEED - START_SPEED);

    // ── Rider animation ──
    rider.position.x = x;
    rider.rotation.z = -steerShown * 0.3;
    rider.position.y = Math.sin(clock * 9) * 0.02 * (0.4 + pace);
    bird.rotation.z = steerShown * 0.12;                       // counter-lean
    bird.rotation.y = -0.3 - steerShown * 0.25;
    // A woodpecker that never pecks is just a red bird.
    headGroup.rotation.x = Math.sin(clock * 11) * 0.09 + 0.04;
    headGroup.rotation.z = steerShown * 0.18;
    tail.rotation.y = Math.sin(clock * 5) * 0.16 - steerShown * 0.2;
    armL.rotation.z = -0.95 - steerShown * 0.5 + Math.sin(clock * 6) * 0.07;
    armR.rotation.z = 0.95 - steerShown * 0.5 - Math.sin(clock * 6 + 1) * 0.07;
    for (const w of board.userData.wheels) w.rotation.x -= speed * dt * 1.6;

    if (crash > 0) {
      rider.rotation.z += crash * 1.5;
      rider.position.y += Math.sin(crash * Math.PI) * 1.2;
      bird.rotation.x = crash * 2.2;
    }

    shadow.position.set(x, 0.03, 0);
    shadow.material.opacity = 0.28 * (1 - crash);

    // ── Camera ──
    // FOV widens with speed; that alone does most of the work of making 60
    // units/sec feel fast. The shake is only on the crash.
    camera.fov = 62 + pace * 7;
    camera.updateProjectionMatrix();
    const jitter = shake * 0.4;
    // High enough and far enough back that the rider sits in the lower third
    // and the road ahead — where the obstacles are — stays unobstructed.
    camera.position.set(
      x * 0.75 + steerShown * 0.5 + rand(-jitter, jitter),
      4.6 + rand(-jitter, jitter),
      9.2 + crash * 1.5);
    camera.lookAt(x * 0.35 + steerShown * 0.9, 1.2, -18);
    camera.rotation.z = -steerShown * 0.06;
    shake = Math.max(0, shake - dt * 2.5);

    sky.position.x = x * 0.15;

    // ── World ──
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
            // A collected star pops and rises instead of blinking out — the
            // reward has to be visible or it does not feel like one.
            p.userData.pop += dt * 3;
            p.position.y += p.userData.pop * 2.2;
            p.scale.setScalar(1 + p.userData.pop * 1.6);
            if (p.userData.pop > 1) { p.visible = false; p.userData.pop = 0; p.scale.setScalar(1); }
            continue;
          }
        }
        // World position along z; the rider sits at z = 0.
        const pz = slab.position.z + p.position.z;
        if (alive && pz > -1.4 && pz < 1.4 && Math.abs(p.position.x - x) < 1.0 && p.visible) {
          if (p.userData.kind === 'star') {
            p.userData.pop = 0.001;
            stars += 1;
            onStar(stars);
          } else {
            finish();
          }
        }
        if (pz < -SEGMENT * 2) { p.visible = true; p.userData.pop = 0; p.scale.setScalar(1); }
      }
    }

    // ── Dust ──
    if (alive) spawnDust(dt);
    for (let i = 0; i < DUST; i++) {
      const m = motes[i];
      if (m.life <= 0) { dummy.scale.setScalar(0); dummy.position.set(0, -50, 0); }
      else {
        m.life -= dt * 1.6;
        m.x += m.vx * dt;
        m.y += m.vy * dt;
        m.vy -= 3.2 * dt;
        m.z += speed * dt * 0.55;
        dummy.position.set(m.x, Math.max(0.06, m.y), m.z);
        dummy.scale.setScalar(Math.max(0, m.life) * m.scale);
      }
      dummy.rotation.set(m.life * 4, m.life * 3, 0);
      dummy.updateMatrix();
      dust.setMatrixAt(i, dummy.matrix);
    }
    dust.instanceMatrix.needsUpdate = true;

    if (alive) onTick({ distance: Math.floor(travelled), stars, speed: Math.round(speed) });
    renderer.render(scene, camera);
  }
  raf = requestAnimationFrame(frame);

  return function stop() {
    alive = false;
    cancelAnimationFrame(raf);
    el.removeEventListener('touchstart', onDown);
    el.removeEventListener('touchmove', onMove);
    el.removeEventListener('touchend', onUp);
    el.removeEventListener('mousedown', onDown);
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
    window.removeEventListener('keydown', keyDown);
    window.removeEventListener('keyup', keyUp);
    window.removeEventListener('resize', onResize);
    if (ro) ro.disconnect();
    // Without this the GL context leaks; a few screen visits and the browser
    // starts refusing to create new ones.
    renderer.dispose();
    if (el.parentNode) el.parentNode.removeChild(el);
  };
}
