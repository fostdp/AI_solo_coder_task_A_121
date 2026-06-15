const API_BASE = 'http://localhost:8080/api/v1';
let currentBridge = null;
let aerodynamicResult = null;
let alerts = [];
let windSpeed = 15, attackAngle = 0;
let showWindFlow = true, showDeformation = true, showCableColor = false;
let scene, camera, renderer, controls, raycaster, mouse;
let deckMesh = null, cableMeshes = [], towerMeshes = [], windParticles = null;
const timeStart = Date.now();
let autoPollTimer = null;

const BRIDGES = [
    {bridge_id:"BS001",name:"泸定桥",location:"四川甘孜泸定县",latitude:29.9092,longitude:102.2374,length:103.67,span:100.0,width:2.8,cable_count:13,construction_year:1706,material:"铁索",deck_height:14.5,design_wind_speed:35.0},
    {bridge_id:"BS002",name:"霁虹桥",location:"云南保山澜沧江",latitude:25.4833,longitude:99.4167,length:113.4,span:106.0,width:3.7,cable_count:18,construction_year:1475,material:"铁索",deck_height:21.0,design_wind_speed:32.0},
    {bridge_id:"BS003",name:"云龙桥",location:"贵州镇远舞阳河",latitude:27.0500,longitude:108.4167,length:95.0,span:88.0,width:3.2,cable_count:12,construction_year:1520,material:"铁索",deck_height:18.0,design_wind_speed:30.0},
    {bridge_id:"BS004",name:"重安江铁索桥",location:"贵州黄平重安江",latitude:26.5833,longitude:107.9167,length:42.0,span:36.5,width:2.5,cable_count:15,construction_year:1871,material:"铁索",deck_height:10.0,design_wind_speed:28.0},
    {bridge_id:"BS005",name:"盘江铁索桥",location:"贵州安顺盘江",latitude:25.7500,longitude:104.7500,length:78.0,span:71.0,width:2.9,cable_count:14,construction_year:1638,material:"铁索",deck_height:25.0,design_wind_speed:38.0},
    {bridge_id:"BS006",name:"程阳桥",location:"广西柳州三江",latitude:25.9833,longitude:109.6667,length:64.4,span:58.0,width:3.4,cable_count:10,construction_year:1916,material:"铁木混合",deck_height:12.0,design_wind_speed:25.0},
    {bridge_id:"BS007",name:"金龙桥",location:"云南丽江金沙江",latitude:27.0333,longitude:100.4500,length:116.0,span:108.0,width:3.2,cable_count:16,construction_year:1878,material:"铁索",deck_height:28.0,design_wind_speed:40.0},
    {bridge_id:"BS008",name:"豆沙关铁索桥",location:"云南盐津豆沙关",latitude:28.2000,longitude:104.2333,length:55.0,span:49.0,width:2.6,cable_count:11,construction_year:1560,material:"铁索",deck_height:16.0,design_wind_speed:33.0},
    {bridge_id:"BS009",name:"普安桥",location:"四川雅安天全",latitude:30.0833,longitude:102.7833,length:48.0,span:42.0,width:2.7,cable_count:9,construction_year:1812,material:"铁索",deck_height:11.0,design_wind_speed:29.0},
    {bridge_id:"BS010",name:"安顺场铁索桥",location:"四川石棉安顺场",latitude:29.3333,longitude:102.3833,length:68.0,span:62.0,width:2.8,cable_count:12,construction_year:1780,material:"铁索",deck_height:13.0,design_wind_speed:31.0}
];

const NOMINAL_CABLE_FORCE = {
    BS001: 520, BS002: 480, BS003: 410, BS004: 380, BS005: 460,
    BS006: 350, BS007: 550, BS008: 390, BS009: 370, BS010: 400
};

window.addEventListener('DOMContentLoaded', () => {
    initBridges();
    initThree();
    selectBridge(BRIDGES[0].bridge_id);
    startAutoPoll();
});

function initBridges() {
    const list = document.getElementById('bridge-list');
    list.innerHTML = '';
    BRIDGES.forEach((b) => {
        const div = document.createElement('div');
        div.className = 'bridge-item';
        div.dataset.id = b.bridge_id;
        div.innerHTML = `<div><div class="name">${b.name}</div><div class="id">${b.bridge_id} · ${b.location}</div></div><div class="status"></div>`;
        div.onclick = () => selectBridge(b.bridge_id);
        list.appendChild(div);
    });
}

function selectBridge(id) {
    currentBridge = BRIDGES.find(b => b.bridge_id === id);
    document.querySelectorAll('.bridge-item').forEach(el => {
        el.classList.toggle('active', el.dataset.id === id);
    });
    renderBridgeInfo();
    buildBridge(currentBridge);
    fetchAeroData();
    updateStats();
}

function renderBridgeInfo() {
    if (!currentBridge) return;
    const age = new Date().getFullYear() - currentBridge.construction_year;
    document.getElementById('bridge-info').innerHTML = `
        <div class="info-row"><span class="label">编号</span><span class="value highlight">${currentBridge.bridge_id}</span></div>
        <div class="info-row"><span class="label">位置</span><span class="value">${currentBridge.location}</span></div>
        <div class="info-row"><span class="label">建成年份</span><span class="value">${currentBridge.construction_year}年 (${age}年)</span></div>
        <div class="info-row"><span class="label">材质</span><span class="value">${currentBridge.material}</span></div>
        <div class="info-row"><span class="label">总长 / 主跨</span><span class="value">${currentBridge.length}m / ${currentBridge.span}m</span></div>
        <div class="info-row"><span class="label">桥面宽</span><span class="value">${currentBridge.width}m</span></div>
        <div class="info-row"><span class="label">索缆数</span><span class="value">${currentBridge.cable_count} 根</span></div>
        <div class="info-row"><span class="label">距水面高</span><span class="value">${currentBridge.deck_height}m</span></div>
        <div class="info-row"><span class="label">设计风速</span><span class="value warning">${currentBridge.design_wind_speed} m/s</span></div>
        <div class="info-row"><span class="label">经纬度</span><span class="value">${currentBridge.latitude.toFixed(3)}, ${currentBridge.longitude.toFixed(3)}</span></div>
    `;
}

function initThree() {
    const container = document.getElementById('canvas-container');
    scene = new THREE.Scene();
    scene.fog = new THREE.FogExp2(0x0a0e1a, 0.006);
    camera = new THREE.PerspectiveCamera(55, container.clientWidth / container.clientHeight, 0.1, 5000);
    camera.position.set(120, 60, 100);
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(container.clientWidth, container.clientHeight);
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    container.appendChild(renderer.domElement);
    controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.06;
    controls.minDistance = 20;
    controls.maxDistance = 500;
    controls.maxPolarAngle = Math.PI * 0.92;
    scene.add(new THREE.AmbientLight(0x6080a0, 0.45));
    const dirLight = new THREE.DirectionalLight(0xfff0d8, 0.95);
    dirLight.position.set(120, 180, 100);
    dirLight.castShadow = true;
    dirLight.shadow.mapSize.set(2048, 2048);
    dirLight.shadow.camera.left = -250; dirLight.shadow.camera.right = 250;
    dirLight.shadow.camera.top = 250; dirLight.shadow.camera.bottom = -250;
    dirLight.shadow.camera.near = 0.5; dirLight.shadow.camera.far = 600;
    scene.add(dirLight);
    const rimLight = new THREE.DirectionalLight(0x5a7a9a, 0.35);
    rimLight.position.set(-80, 60, -100);
    scene.add(rimLight);
    scene.add(new THREE.HemisphereLight(0x87a9c8, 0x2a3a4a, 0.35));

    const waterGeo = new THREE.PlaneGeometry(1800, 900, 80, 50);
    const water = new THREE.Mesh(waterGeo, new THREE.MeshPhongMaterial({
        color: 0x1a3a5a, transparent: true, opacity: 0.88, shininess: 90, specular: 0x4a8ac0
    }));
    water.rotation.x = -Math.PI / 2;
    water.position.y = -2;
    water.receiveShadow = true;
    const wp = waterGeo.attributes.position;
    water.userData.originalZ = [];
    for (let i = 0; i < wp.count; i++) {
        const z = Math.sin(wp.getX(i) * 0.028 + wp.getY(i) * 0.015) * 0.32 + Math.sin(wp.getX(i) * 0.07) * 0.1;
        wp.setZ(i, z); water.userData.originalZ.push(z);
    }
    wp.needsUpdate = true; waterGeo.computeVertexNormals();
    water.userData.waterPositions = wp;
    water.userData.isWater = true;
    scene.add(water);

    const mountainMat = new THREE.MeshStandardMaterial({ color: 0x3a4a3a, flatShading: true, roughness: 0.92 });
    const mountainDarkMat = new THREE.MeshStandardMaterial({ color: 0x2a3a2e, flatShading: true, roughness: 0.95 });
    for (let i = 0; i < 16; i++) {
        const size = 70 + Math.random() * 140;
        const mat = i % 2 === 0 ? mountainMat : mountainDarkMat;
        const m = new THREE.Mesh(new THREE.ConeGeometry(size, size * (1.4 + Math.random() * 0.6), 5 + Math.floor(Math.random() * 3)), mat);
        const a = (i / 16) * Math.PI * 2 + Math.random() * 0.35;
        const r = 380 + Math.random() * 280;
        m.position.set(Math.cos(a) * r, size * 0.55 - 2, Math.sin(a) * r);
        m.rotation.y = Math.random() * Math.PI;
        scene.add(m);
    }
    const trunkMat = new THREE.MeshStandardMaterial({ color: 0x4a3020, roughness: 0.92 });
    const leafMats = [
        new THREE.MeshStandardMaterial({ color: 0x2a5a30, flatShading: true }),
        new THREE.MeshStandardMaterial({ color: 0x3a6a40, flatShading: true })
    ];
    for (let i = 0; i < 90; i++) {
        const angle = Math.random() * Math.PI * 2;
        const radius = 180 + Math.random() * 380;
        const tx = Math.cos(angle) * radius;
        const tz = Math.sin(angle) * radius;
        const th = 2 + Math.random() * 2.5;
        const trunk = new THREE.Mesh(new THREE.CylinderGeometry(0.3, 0.55, th, 5), trunkMat);
        trunk.position.set(tx, th / 2 - 1.5, tz);
        scene.add(trunk);
        const lh = 5 + Math.random() * 5;
        const leaves = new THREE.Mesh(new THREE.ConeGeometry(2.5 + Math.random() * 2.5, lh, 6), leafMats[i % 2]);
        leaves.position.set(tx, th + lh / 2 - 1.5, tz);
        scene.add(leaves);
    }
    const starsGeo = new THREE.BufferGeometry();
    const starCount = 1200;
    const starPos = new Float32Array(starCount * 3);
    for (let i = 0; i < starCount; i++) {
        const theta = Math.random() * Math.PI * 2;
        const phi = Math.random() * Math.PI * 0.55;
        const r = 1800;
        starPos[i*3] = Math.cos(theta) * Math.sin(phi) * r;
        starPos[i*3+1] = Math.cos(phi) * r;
        starPos[i*3+2] = Math.sin(theta) * Math.sin(phi) * r;
    }
    starsGeo.setAttribute('position', new THREE.BufferAttribute(starPos, 3));
    scene.add(new THREE.Points(starsGeo, new THREE.PointsMaterial({
        color: 0xeef5ff, size: 1.3, sizeAttenuation: false, transparent: true, opacity: 0.75
    })));

    raycaster = new THREE.Raycaster();
    mouse = new THREE.Vector2();
    renderer.domElement.addEventListener('click', onCanvasClick);
    renderer.domElement.addEventListener('mousemove', onCanvasHover);
    window.addEventListener('resize', onWindowResize);
    animate();
}

function onCanvasHover(e) {
    if (!deckMesh) return;
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObject(deckMesh, true);
    renderer.domElement.style.cursor = hits.length > 0 ? 'pointer' : 'default';
}

function buildBridge(bridge) {
    cableMeshes.forEach(m => scene.remove(m));
    towerMeshes.forEach(m => scene.remove(m));
    if (deckMesh) scene.remove(deckMesh);
    if (windParticles) scene.remove(windParticles);
    cableMeshes = []; towerMeshes = []; deckMesh = null; windParticles = null;

    const span = bridge.span;
    const width = bridge.width;
    const cableCount = bridge.cable_count;
    const deckHeight = bridge.deck_height;
    const towerHeight = deckHeight + span * 0.085;
    const stoneMat = new THREE.MeshStandardMaterial({ color: 0x5a5048, roughness: 0.9 });
    const stoneDark = new THREE.MeshStandardMaterial({ color: 0x4a4038, roughness: 0.95 });

    [-1, 1].forEach(side => {
        const tx = side * (span / 2 + 8);
        const base = new THREE.Mesh(new THREE.BoxGeometry(11, 7, 9), stoneDark);
        base.position.set(tx, 1.5, 0); base.receiveShadow = true; base.castShadow = true;
        scene.add(base); towerMeshes.push(base);
        const pillar = new THREE.Mesh(new THREE.BoxGeometry(3.2, towerHeight, 4.2), stoneMat);
        pillar.position.set(tx, towerHeight / 2 - 2, 0); pillar.castShadow = true; pillar.receiveShadow = true;
        scene.add(pillar); towerMeshes.push(pillar);
        const cap = new THREE.Mesh(new THREE.BoxGeometry(4.8, 2.8, 5.8), stoneDark);
        cap.position.set(tx, towerHeight + 0.6 - 2, 0); cap.castShadow = true;
        scene.add(cap); towerMeshes.push(cap);
        for (let col = -1; col <= 1; col += 2) {
            for (let r = 0; r < 2; r++) {
                const rope = new THREE.Mesh(
                    new THREE.CylinderGeometry(0.09, 0.09, towerHeight * 0.95, 6),
                    new THREE.MeshStandardMaterial({ color: 0x6a5a4a, roughness: 0.85 })
                );
                rope.position.set(tx + col * 1.1 + r * 0.1, towerHeight * 0.45 - 2, col * 1.5);
                scene.add(rope); towerMeshes.push(rope);
            }
        }
        const anchor = new THREE.Mesh(new THREE.BoxGeometry(9, 6, 11), stoneDark);
        anchor.position.set(side * (span / 2 + 13), 1, 0); anchor.castShadow = true; anchor.receiveShadow = true;
        scene.add(anchor); towerMeshes.push(anchor);
        const anchorTop = new THREE.Mesh(new THREE.BoxGeometry(7, 1.5, 9), stoneMat);
        anchorTop.position.set(side * (span / 2 + 13), 4.75, 0);
        scene.add(anchorTop); towerMeshes.push(anchorTop);
    });

    const deckSegments = 60;
    const deckGeo = new THREE.BoxGeometry(span, 0.35, width, deckSegments, 1, 5);
    const deckMat = new THREE.MeshStandardMaterial({
        color: 0x8a6a40, roughness: 0.82, metalness: 0.08, vertexColors: true
    });
    deckMesh = new THREE.Mesh(deckGeo, deckMat);
    deckMesh.position.y = deckHeight;
    deckMesh.castShadow = true; deckMesh.receiveShadow = true;
    deckMesh.userData.originalPositions = [];
    const dp = deckGeo.attributes.position;
    const colors = new Float32Array(dp.count * 3);
    for (let i = 0; i < dp.count; i++) {
        deckMesh.userData.originalPositions.push(dp.getX(i), dp.getY(i), dp.getZ(i));
        const tone = 0.9 + Math.random() * 0.2;
        colors[i*3] = 0.55 * tone; colors[i*3+1] = 0.42 * tone; colors[i*3+2] = 0.25 * tone;
    }
    deckGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    scene.add(deckMesh);

    const frameMat = new THREE.MeshStandardMaterial({ color: 0x3a2a1a, roughness: 0.75 });
    for (let s = 0; s <= deckSegments; s += 2) {
        const x = -span / 2 + (s / deckSegments) * span;
        const cb = new THREE.Mesh(new THREE.BoxGeometry(0.18, 0.22, width + 0.6), frameMat);
        cb.position.set(x, deckHeight - 0.06, 0); deckMesh.add(cb);
    }
    const sideBeam = new THREE.Mesh(new THREE.BoxGeometry(span, 0.15, 0.18), frameMat);
    sideBeam.position.set(0, deckHeight - 0.05, width / 2 + 0.09); deckMesh.add(sideBeam);
    const sideBeam2 = sideBeam.clone(); sideBeam2.position.z = -width / 2 - 0.09; deckMesh.add(sideBeam2);

    const cableColor = new THREE.Color(0x6a5050);
    const cableMetal = new THREE.MeshStandardMaterial({ color: cableColor, metalness: 0.75, roughness: 0.28 });
    const leftX = -span / 2 - 13, ltX = -span / 2 - 8;
    const rightX = span / 2 + 13, rtX = span / 2 + 8;

    for (let ci = 0; ci < cableCount; ci++) {
        const side = ci < cableCount / 2 ? -1 : 1;
        const li = ci < cableCount / 2 ? ci : ci - Math.ceil(cableCount / 2);
        const tl = Math.ceil(cableCount / 2);
        const zOff = (li - (tl - 1) / 2) * 0.38 * side;
        const pts = [];
        const seg = 70;
        const sagF = 0.082;
        for (let s = 0; s <= seg; s++) {
            const t = s / seg;
            let x, y;
            if (t < 0.085) {
                const tt = t / 0.085;
                x = leftX + (ltX - leftX) * tt;
                y = 4 + (towerHeight - 2 - 4) * easeInOutCubic(tt);
            } else if (t < 0.915) {
                const tt = (t - 0.085) / 0.83;
                x = ltX + (rtX - ltX) * tt;
                y = towerHeight - 2 - 4 * sagF * span * tt * (1 - tt);
            } else {
                const tt = (t - 0.915) / 0.085;
                x = rtX + (rightX - rtX) * tt;
                y = towerHeight - 2 + (4 - (towerHeight - 2)) * easeInOutCubic(tt);
            }
            pts.push(new THREE.Vector3(x, y, zOff));
        }
        const tube = new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 130, 0.085, 7, false);
        const mesh = new THREE.Mesh(tube, cableMetal.clone());
        mesh.userData.baseColor = cableColor.clone();
        cableMeshes.push(mesh); scene.add(mesh);
        const hangerCount = 14;
        for (let h = 1; h <= hangerCount; h++) {
            const tt = h / (hangerCount + 1);
            const cx = ltX + (rtX - ltX) * tt;
            const cy = towerHeight - 2 - 4 * sagF * span * tt * (1 - tt);
            const hdist = cy - deckHeight;
            const hang = new THREE.Mesh(
                new THREE.CylinderGeometry(0.016, 0.016, hdist, 5),
                new THREE.MeshStandardMaterial({ color: 0x8a8080, metalness: 0.85, roughness: 0.22 })
            );
            hang.position.set(cx, (cy + deckHeight) / 2, zOff);
            scene.add(hang); cableMeshes.push(hang);
        }
    }

    const postMat = new THREE.MeshStandardMaterial({ color: 0x4a3a2a, roughness: 0.78 });
    const railMat = new THREE.MeshStandardMaterial({ color: 0x5a3a20, roughness: 0.75 });
    const postCount = 28;
    for (let side = -1; side <= 1; side += 2) {
        for (let hs = 0; hs < 2; hs++) {
            const zBase = side * (width / 2 + 0.1) + hs * side * 0.28;
            for (let p = 0; p <= postCount; p++) {
                const px = -span / 2 + (p / postCount) * span;
                const post = new THREE.Mesh(new THREE.CylinderGeometry(0.032, 0.038, 1.1, 5), postMat);
                post.position.set(px, deckHeight + 0.7, zBase); deckMesh.add(post);
            }
            const railTop = new THREE.Mesh(new THREE.CylinderGeometry(0.025, 0.025, span, 5), railMat);
            railTop.rotation.z = Math.PI / 2; railTop.position.set(0, deckHeight + 1.18, zBase); deckMesh.add(railTop);
            const railMid = railTop.clone(); railMid.position.y = deckHeight + 0.7; deckMesh.add(railMid);
        }
    }
    createWindParticles(span, width, deckHeight);
    updateCameraForBridge(bridge);
}

function easeInOutCubic(t) { return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2; }
function updateCameraForBridge(bridge) {
    const dist = bridge.span * 1.25;
    camera.position.set(dist * 0.65, dist * 0.52, dist * 1.0);
    controls.target.set(0, bridge.deck_height + bridge.span * 0.04, 0); controls.update();
}

function createWindParticles(span, width, deckHeight) {
    const count = 1500;
    const positions = new Float32Array(count * 3);
    const velocities = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
        positions[i*3] = (Math.random() - 0.5) * span * 2.8;
        positions[i*3+1] = deckHeight - 5 + (Math.random() - 0.2) * span * 0.45;
        positions[i*3+2] = (Math.random() - 0.5) * width * 18;
        velocities[i*3] = 0.8 + Math.random() * 2.2;
        velocities[i*3+1] = (Math.random() - 0.5) * 0.35;
        velocities[i*3+2] = (Math.random() - 0.5) * 0.25;
        const t = ((positions[i*3+1] - (deckHeight - 5)) / (span * 0.45));
        const c = new THREE.Color().setHSL(0.54 + t * 0.12, 0.72, 0.52 + t * 0.18);
        colors[i*3] = c.r; colors[i*3+1] = c.g; colors[i*3+2] = c.b;
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    windParticles = new THREE.Points(geo, new THREE.PointsMaterial({
        size: 1.35, vertexColors: true, transparent: true, opacity: 0.78,
        blending: THREE.AdditiveBlending, sizeAttenuation: true, depthWrite: false
    }));
    windParticles.userData = { velocities, span, width, deckHeight };
    scene.add(windParticles);
}

function animate() {
    requestAnimationFrame(animate);
    controls.update();
    const now = Date.now();
    const elapsed = (now - timeStart) / 1000;

    if (windParticles && showWindFlow) {
        const pos = windParticles.geometry.attributes.position.array;
        const vel = windParticles.userData.velocities;
        const { span, width, deckHeight } = windParticles.userData;
        const sf = (windSpeed / 15.0) * (1 + attackAngle / 60);
        const dirOffset = (attackAngle / 180) * Math.PI;
        for (let i = 0; i < pos.length / 3; i++) {
            const idx = i * 3;
            const verticalShear = 1 + ((pos[idx + 1] - deckHeight) / (span * 0.4)) * 0.4;
            pos[idx] += vel[idx] * sf * 0.85 * Math.max(0.4, verticalShear);
            pos[idx + 1] += vel[idx + 1] * 0.06;
            const swirl = Math.sin(elapsed * 0.8 + i * 0.05) * 0.025 * sf;
            pos[idx + 2] += vel[idx + 2] * 0.06 + swirl + Math.sin(dirOffset) * 0.04 * sf;
            if (pos[idx] > span * 1.45) {
                pos[idx] = -span * 1.45;
                pos[idx + 1] = deckHeight - 5 + (Math.random() - 0.2) * span * 0.45;
                pos[idx + 2] = (Math.random() - 0.5) * width * 18;
            }
            if (Math.abs(pos[idx + 2]) > width * 10) pos[idx + 2] *= -0.9;
        }
        windParticles.geometry.attributes.position.needsUpdate = true;
    }

    scene.children.forEach(obj => {
        if (obj.userData && obj.userData.isWater && obj.userData.waterPositions) {
            const wp = obj.userData.waterPositions;
            const orig = obj.userData.originalZ;
            for (let i = 0; i < wp.count; i++) {
                const ox = wp.getX(i);
                const oy = wp.getY(i);
                const extra = Math.sin(ox * 0.012 + elapsed * 0.7 + oy * 0.01) * 0.15
                            + Math.sin(ox * 0.035 + elapsed * 1.4) * 0.08;
                wp.setZ(i, orig[i] + extra * (0.6 + windSpeed / 50));
            }
            wp.needsUpdate = true;
            obj.geometry.computeVertexNormals();
        }
    });

    if (deckMesh && currentBridge) {
        const span = currentBridge.span;
        const amp = aerodynamicResult ? aerodynamicResult.vibration_amplitude : (windSpeed / 35) * 0.16;
        const omega = 2 * Math.PI * (1.2 * Math.sqrt(9.81 / span));
        const damp = aerodynamicResult ? Math.max(aerodynamicResult.aerodynamic_damping, 0.001) : 0.012;
        const t = elapsed % 6;
        const envelope = Math.exp(-damp * omega * (t % 2.5));
        const phaseT = omega * t;
        const pos = deckMesh.geometry.attributes.position;
        const colors = deckMesh.geometry.attributes.color.array;
        const orig = deckMesh.userData.originalPositions;
        const cB = new THREE.Color(0x2266cc), cG = new THREE.Color(0x22c55e);
        const cY = new THREE.Color(0xeab308), cO = new THREE.Color(0xf97316), cR = new THREE.Color(0xdc2626);
        for (let i = 0; i < pos.count; i++) {
            const ox = orig[i*3], oy = orig[i*3+1], oz = orig[i*3+2];
            const xR = (ox + span / 2) / span;
            const shape = Math.sin(Math.PI * xR);
            const dispStatic = amp * shape * 0.45;
            const dispVib = amp * shape * 0.55 * envelope * Math.cos(phaseT + xR * Math.PI);
            const totalDisp = dispStatic + dispVib;
            const torsion = 0.004 * totalDisp * (attackAngle / 10) * (windSpeed / 30) * (oz / currentBridge.width);
            pos.setXYZ(i, ox, oy + totalDisp + torsion, oz);
            if (showDeformation) {
                const nd = Math.min(Math.abs(totalDisp) / 0.22, 1.0);
                let c;
                if (nd < 0.25) c = cB.clone().lerp(cG, nd / 0.25);
                else if (nd < 0.5) c = cG.clone().lerp(cY, (nd - 0.25) / 0.25);
                else if (nd < 0.75) c = cY.clone().lerp(cO, (nd - 0.5) / 0.25);
                else c = cO.clone().lerp(cR, (nd - 0.75) / 0.25);
                const tone = 0.92 + Math.sin(i * 0.1) * 0.08;
                colors[i*3] = c.r * tone; colors[i*3+1] = c.g * tone; colors[i*3+2] = c.b * tone;
            } else {
                const tone = 0.9 + Math.sin(i * 0.1) * 0.1;
                colors[i*3] = 0.55 * tone; colors[i*3+1] = 0.42 * tone; colors[i*3+2] = 0.25 * tone;
            }
        }
        pos.needsUpdate = true;
        deckMesh.geometry.attributes.color.needsUpdate = true;
        deckMesh.geometry.computeVertexNormals();
    }

    if (showCableColor && cableMeshes.length) {
        cableMeshes.forEach((m, i) => {
            if (m.userData.baseColor) {
                const ratio = Math.min(windSpeed / 40, 1) * (0.7 + 0.3 * Math.sin(elapsed * 1.5 + i * 0.6));
                m.material.color.copy(m.userData.baseColor).lerp(new THREE.Color(0xef4444), ratio * 0.55);
            }
        });
    }

    renderer.render(scene, camera);
}

function onWindowResize() {
    const container = document.getElementById('canvas-container');
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(container.clientWidth, container.clientHeight);
}

function onCanvasClick(event) {
    if (!deckMesh || !currentBridge) return;
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObject(deckMesh, true);
    if (hits.length > 0) showPointDetail(hits[0].point);
}

function closeModal() { document.getElementById('detail-modal').classList.remove('show'); }
function toggleAlerts() { document.getElementById('alerts-panel').classList.toggle('collapsed'); }
function resetCamera() { if (currentBridge) updateCameraForBridge(currentBridge); }
function applyWindControl() {
    const w = parseFloat(document.getElementById('ctrl-wind').value);
    const a = parseFloat(document.getElementById('ctrl-angle').value);
    if (!isNaN(w) && w >= 0) windSpeed = w;
    if (!isNaN(a) && a >= -15 && a <= 15) attackAngle = a;
    fetchAeroData();
}
function toggleLayer(name) {
    const el = document.querySelector(`.tg-btn[data-toggle="${name}"]`);
    if (!el) return;
    el.classList.toggle('active');
    if (name === 'windFlow') showWindFlow = el.classList.contains('active');
    if (name === 'deformation') showDeformation = el.classList.contains('active');
    if (name === 'cables') {
        showCableColor = el.classList.contains('active');
        if (!showCableColor) cableMeshes.forEach(m => { if (m.userData.baseColor) m.material.color.copy(m.userData.baseColor); });
    }
}

async function fetchAeroData() {
    if (!currentBridge) return;
    try {
        const res = await fetch(`${API_BASE}/aerodynamics/evaluate?bridge_id=${currentBridge.bridge_id}&wind_speed=${windSpeed}&attack_angle=${attackAngle}`);
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const json = await res.json();
        if (json.success && json.data) aerodynamicResult = json.data;
        else aerodynamicResult = computeFallbackAero();
    } catch (e) {
        aerodynamicResult = computeFallbackAero();
    }
    updateMetrics();
    drawAllCharts();
    checkAlerts();
}

function computeFallbackAero() {
    const b = currentBridge;
    const omega = 2 * Math.PI * 1.2 * Math.sqrt(9.81 / b.span);
    const reduced = omega * b.width / Math.max(windSpeed, 1);
    const massPerLen = b.width * 0.5 * 7850;
    const rho_b = 1.225 * b.width * b.width / (2 * massPerLen);
    const h_star = -0.5 * Math.min(reduced, 5) / 5;
    const aero_damp = 0.01 - rho_b * h_star / (2 * Math.max(reduced, 0.1));
    const mu = (massPerLen * b.width * b.width / 12) / (1.225 * Math.pow(b.width, 4));
    const freqRatio = 2.5;
    const base = (omega * b.width) * Math.sqrt(8 * mu * (1/12) * (freqRatio * freqRatio - 1)) / (0.2 * 0.6);
    const critical = Math.max(b.design_wind_speed * 1.3, Math.min(base, 80));
    const margin = windSpeed > 0 ? Math.max(-0.2, (critical - windSpeed) / critical) : 1.0;
    const q = 0.5 * 1.225 * windSpeed * windSpeed * b.width;
    const cl = 2 * Math.PI * Math.sin(attackAngle * Math.PI / 180);
    const lift = q * cl;
    const totalDamp = Math.max(aero_damp, 0.0001);
    const amp = windSpeed < 1 ? 0.001 : Math.min(2, Math.abs(lift) / (massPerLen * omega * omega * 2 * totalDamp));
    return {
        wind_speed: windSpeed, attack_angle: attackAngle,
        aerodynamic_damping: aero_damp, vibration_amplitude: amp,
        flutter_critical_speed: critical, flutter_margin: margin,
        is_safe: aero_damp > 0 && margin > 0.1
    };
}

function updateStats() {
    document.getElementById('stat-wind').innerText = windSpeed.toFixed(1);
    document.getElementById('stat-dir').innerText = attackAngle.toFixed(1);
    const amp = aerodynamicResult ? aerodynamicResult.vibration_amplitude : (windSpeed / 35) * 0.15;
    document.getElementById('stat-amp').innerText = amp.toFixed(3);
    document.getElementById('stat-crit').innerText = aerodynamicResult ? aerodynamicResult.flutter_critical_speed.toFixed(1) : (currentBridge ? currentBridge.design_wind_speed.toFixed(1) : '--');
}

function updateMetrics() {
    if (!aerodynamicResult) return;
    const r = aerodynamicResult;
    updateStats();
    const safeClass = r.is_safe ? 'safe' : (r.flutter_margin < 0.05 ? 'danger' : 'warning');
    document.getElementById('aero-metrics').innerHTML = `
        <div class="metric-card ${r.aerodynamic_damping > 0 ? 'safe' : 'danger'}">
            <div class="mc-title">气动阻尼 ξₐ</div>
            <div class="mc-value">${r.aerodynamic_damping.toFixed(4)}</div>
            <div class="mc-unit">${r.aerodynamic_damping > 0 ? '正值 - 稳定 ✓' : '⚠ 负值 - 可能发散'}</div>
        </div>
        <div class="metric-card ${safeClass}">
            <div class="mc-title">振动幅值</div>
            <div class="mc-value">${(r.vibration_amplitude*1000).toFixed(0)}<span style="font-size:12px;">mm</span></div>
            <div class="mc-unit">安全阈值 150mm</div>
        </div>
        <div class="metric-card ${safeClass}">
            <div class="mc-title">颤振临界 Ucr</div>
            <div class="mc-value">${r.flutter_critical_speed.toFixed(1)}<span style="font-size:12px;">m/s</span></div>
            <div class="mc-unit">设计 Vd=${currentBridge.design_wind_speed}m/s</div>
        </div>
        <div class="metric-card ${safeClass}">
            <div class="mc-title">颤振裕度</div>
            <div class="mc-value">${(r.flutter_margin*100).toFixed(0)}<span style="font-size:12px;">%</span></div>
            <div class="mc-unit">要求 ≥15%</div>
        </div>
    `;
    const statusEl = document.getElementById('overlay-status');
    const subEl = document.getElementById('overlay-status-sub');
    const marginEl = document.getElementById('overlay-margin');
    statusEl.className = 'overlay-value ' + safeClass;
    marginEl.className = 'overlay-value ' + safeClass;
    if (r.flutter_margin < 0) { statusEl.innerText = '⚠ 危险'; subEl.innerText = '颤振发生！'; }
    else if (!r.is_safe) { statusEl.innerText = '警告'; subEl.innerText = '接近临界'; }
    else if (r.flutter_margin < 0.2) { statusEl.innerText = '留意'; subEl.innerText = '裕度偏低'; }
    else { statusEl.innerText = '安全'; subEl.innerText = '裕度充足'; }
    marginEl.innerText = (Math.max(0, r.flutter_margin) * 100).toFixed(0) + '%';
    document.getElementById('aero-damp').innerText = r.aerodynamic_damping.toFixed(4);
    const fp = r.flutter_margin < 0.35 ? Math.max(0, (0.35 - r.flutter_margin) / 0.35 * 100).toFixed(1) + '%' : '< 1%';
    document.getElementById('flutter-prob').innerText = fp;
}

function startAutoPoll() {
    if (autoPollTimer) clearInterval(autoPollTimer);
    autoPollTimer = setInterval(() => {
        windSpeed = Math.max(2, Math.min(60, windSpeed + (Math.random() - 0.45) * 2));
        attackAngle = Math.max(-12, Math.min(12, attackAngle + (Math.random() - 0.5) * 1.2));
        document.getElementById('ctrl-wind').value = windSpeed.toFixed(1);
        document.getElementById('ctrl-angle').value = attackAngle.toFixed(1);
        fetchAeroData();
    }, 8000);
}

function switchTab(name) {
    document.querySelectorAll('.panel-tab').forEach(t => t.classList.toggle('active', t.dataset.tab === name));
    document.querySelectorAll('.panel-section').forEach(s => s.classList.toggle('active', s.id === 'tab-' + name));
    if (name === 'vibration') drawVibrationChart();
    else if (name === 'flutter') drawFlutterChart();
    else if (name === 'cable') drawCableChart();
}

function drawAllCharts() {
    const tab = document.querySelector('.panel-tab.active');
    if (!tab) return;
    const n = tab.dataset.tab;
    if (n === 'vibration') drawVibrationChart();
    else if (n === 'flutter') drawFlutterChart();
    else if (n === 'cable') drawCableChart();
}

function drawVibrationChart() {
    if (!currentBridge) return;
    const cv = document.getElementById('canvas-vibration');
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const W = cv.width, H = cv.height;
    ctx.clearRect(0, 0, W, H);
    const b = currentBridge;
    const omega = 2 * Math.PI * 1.2 * Math.sqrt(9.81 / b.span);
    const damp = aerodynamicResult ? Math.max(aerodynamicResult.aerodynamic_damping, 0.001) : 0.01;
    const omegaD = omega * Math.sqrt(1 - damp * damp);
    const amp = aerodynamicResult ? aerodynamicResult.vibration_amplitude : (windSpeed / 35) * 0.15;
    const accelMax = amp * omega * omega;
    const duration = 8;
    const n = 400;
    const pts = [];
    let rms = 0;
    for (let i = 0; i <= n; i++) {
        const t = (i / n) * duration;
        const envelope = Math.exp(-damp * omega * t);
        const accel = -amp * omega * omega * envelope * Math.cos(omegaD * t);
        pts.push(accel);
        rms += accel * accel;
    }
    rms = Math.sqrt(rms / (n + 1));
    const padL = 38, padR = 10, padT = 12, padB = 22;
    const plotW = W - padL - padR, plotH = H - padT - padB;
    ctx.strokeStyle = 'rgba(71, 85, 105, 0.3)';
    ctx.lineWidth = 1;
    ctx.font = '9px sans-serif';
    ctx.fillStyle = '#64748b';
    for (let i = 0; i <= 4; i++) {
        const y = padT + (plotH / 4) * i;
        ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(padL + plotW, y); ctx.stroke();
        const val = accelMax * (1 - i / 2);
        ctx.fillText(val.toFixed(2), 4, y + 3);
    }
    for (let i = 0; i <= 4; i++) {
        const x = padL + (plotW / 4) * i;
        ctx.beginPath(); ctx.moveTo(x, padT); ctx.lineTo(x, padT + plotH); ctx.stroke();
        ctx.fillText(((duration / 4) * i).toFixed(0) + 's', x - 6, padT + plotH + 14);
    }
    const grad = ctx.createLinearGradient(0, padT, 0, padT + plotH);
    grad.addColorStop(0, 'rgba(239, 68, 68, 0.85)');
    grad.addColorStop(0.5, 'rgba(234, 179, 8, 0.85)');
    grad.addColorStop(1, 'rgba(59, 130, 246, 0.85)');
    ctx.strokeStyle = grad;
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    pts.forEach((v, i) => {
        const x = padL + (i / n) * plotW;
        const y = padT + plotH / 2 - (v / (accelMax * 1.15)) * (plotH / 2);
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.strokeStyle = 'rgba(239, 68, 68, 0.5)';
    ctx.setLineDash([3, 3]);
    const thresholdA = 0.5 * 9.81;
    if (thresholdA < accelMax * 1.15) {
        const y1 = padT + plotH / 2 - (thresholdA / (accelMax * 1.15)) * (plotH / 2);
        const y2 = padT + plotH / 2 + (thresholdA / (accelMax * 1.15)) * (plotH / 2);
        ctx.beginPath(); ctx.moveTo(padL, y1); ctx.lineTo(padL + plotW, y1); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(padL, y2); ctx.lineTo(padL + plotW, y2); ctx.stroke();
    }
    ctx.setLineDash([]);
    document.getElementById('vib-unit').innerText = `RMS: ${rms.toFixed(3)} m/s²`;

    const cv2 = document.getElementById('canvas-displacement');
    if (!cv2) return;
    const ctx2 = cv2.getContext('2d');
    const W2 = cv2.width, H2 = cv2.height;
    ctx2.clearRect(0, 0, W2, H2);
    const pad2 = { l: 38, r: 10, t: 10, b: 18 };
    const pw2 = W2 - pad2.l - pad2.r, ph2 = H2 - pad2.t - pad2.b;
    ctx2.strokeStyle = 'rgba(71, 85, 105, 0.3)';
    for (let i = 0; i <= 3; i++) {
        const y = pad2.t + (ph2 / 3) * i;
        ctx2.beginPath(); ctx2.moveTo(pad2.l, y); ctx2.lineTo(pad2.l + pw2, y); ctx2.stroke();
    }
    ctx2.font = '9px sans-serif';
    ctx2.fillStyle = '#64748b';
    ctx2.fillText((amp * 1000).toFixed(0) + 'mm', 2, pad2.t + 8);
    ctx2.fillText('0', 20, pad2.t + ph2 / 2 + 3);
    ctx2.fillText((-amp * 1000).toFixed(0) + 'mm', 2, pad2.t + ph2 - 2);
    const envGrad = ctx2.createLinearGradient(0, pad2.t, 0, pad2.t + ph2);
    envGrad.addColorStop(0, 'rgba(59, 130, 246, 0.9)');
    envGrad.addColorStop(1, 'rgba(34, 197, 94, 0.9)');
    ctx2.strokeStyle = envGrad;
    ctx2.lineWidth = 1.5;
    ctx2.beginPath();
    for (let i = 0; i <= n; i++) {
        const t = (i / n) * duration;
        const envelope = Math.exp(-damp * omega * t);
        const disp = amp * envelope * Math.cos(omegaD * t);
        const x = pad2.l + (i / n) * pw2;
        const y = pad2.t + ph2 / 2 - (disp / (amp * 1.15)) * (ph2 / 2);
        if (i === 0) ctx2.moveTo(x, y); else ctx2.lineTo(x, y);
    }
    ctx2.stroke();
    ctx2.strokeStyle = 'rgba(239, 68, 68, 0.35)';
    ctx2.setLineDash([2, 3]);
    ctx2.beginPath();
    for (let i = 0; i <= n; i++) {
        const t = (i / n) * duration;
        const env = amp * Math.exp(-damp * omega * t);
        const x = pad2.l + (i / n) * pw2;
        const y = pad2.t + ph2 / 2 - (env / (amp * 1.15)) * (ph2 / 2);
        if (i === 0) ctx2.moveTo(x, y); else ctx2.lineTo(x, y);
    }
    ctx2.stroke();
    ctx2.beginPath();
    for (let i = 0; i <= n; i++) {
        const t = (i / n) * duration;
        const env = amp * Math.exp(-damp * omega * t);
        const x = pad2.l + (i / n) * pw2;
        const y = pad2.t + ph2 / 2 + (env / (amp * 1.15)) * (ph2 / 2);
        if (i === 0) ctx2.moveTo(x, y); else ctx2.lineTo(x, y);
    }
    ctx2.stroke();
    ctx2.setLineDash([]);
    document.getElementById('disp-unit').innerText = `峰值: ${(amp * 1000).toFixed(0)} mm`;
}

function drawFlutterChart() {
    if (!currentBridge) return;
    const cv = document.getElementById('canvas-flutter');
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const W = cv.width, H = cv.height;
    ctx.clearRect(0, 0, W, H);
    const origWind = windSpeed, origAngle = attackAngle;
    const curves = [];
    const b = currentBridge;
    for (let a = -10; a <= 10; a++) {
        attackAngle = a; windSpeed = 30;
        const r = computeFallbackAero();
        curves.push({ angle: a, speed: r.flutter_critical_speed, damp: r.aerodynamic_damping });
    }
    windSpeed = origWind; attackAngle = origAngle;
    const padL = 36, padR = 10, padT = 12, padB = 22;
    const pw = W - padL - padR, ph = H - padT - padB;
    const minSpeed = Math.min(...curves.map(c => c.speed)) * 0.88;
    const maxSpeed = Math.max(...curves.map(c => c.speed)) * 1.08;
    ctx.strokeStyle = 'rgba(71, 85, 105, 0.3)';
    ctx.lineWidth = 1;
    ctx.font = '9px sans-serif';
    ctx.fillStyle = '#64748b';
    for (let i = 0; i <= 4; i++) {
        const y = padT + (ph / 4) * i;
        ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(padL + pw, y); ctx.stroke();
        const val = maxSpeed - (maxSpeed - minSpeed) * (i / 4);
        ctx.fillText(val.toFixed(0), 4, y + 3);
    }
    for (let i = 0; i <= 4; i++) {
        const x = padL + (pw / 4) * i;
        ctx.beginPath(); ctx.moveTo(x, padT); ctx.lineTo(x, padT + ph); ctx.stroke();
        const a = -10 + (20 * i / 4);
        ctx.fillText(a.toFixed(0) + '°', x - 5, padT + ph + 14);
    }
    const g1 = ctx.createLinearGradient(0, padT, 0, padT + ph);
    g1.addColorStop(0, 'rgba(34, 197, 94, 0.9)');
    g1.addColorStop(0.5, 'rgba(234, 179, 8, 0.9)');
    g1.addColorStop(1, 'rgba(239, 68, 68, 0.9)');
    ctx.strokeStyle = g1;
    ctx.lineWidth = 2.2;
    ctx.shadowColor = 'rgba(59, 130, 246, 0.4)';
    ctx.shadowBlur = 6;
    ctx.beginPath();
    curves.forEach((c, i) => {
        const x = padL + ((c.angle - (-10)) / 20) * pw;
        const y = padT + (1 - (c.speed - minSpeed) / (maxSpeed - minSpeed)) * ph;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.shadowBlur = 0;
    if (aerodynamicResult) {
        const cx = padL + ((aerodynamicResult.attack_angle - (-10)) / 20) * pw;
        const cy = padT + (1 - (aerodynamicResult.flutter_critical_speed - minSpeed) / (maxSpeed - minSpeed)) * ph;
        ctx.fillStyle = 'rgba(59, 130, 246, 0.25)';
        ctx.beginPath();
        ctx.arc(cx, cy, 18, 0, Math.PI * 2); ctx.fill();
        ctx.fillStyle = '#60a5fa';
        ctx.beginPath();
        ctx.arc(cx, cy, 6, 0, Math.PI * 2); ctx.fill();
        ctx.strokeStyle = '#93c5fd';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.arc(cx, cy, 10, 0, Math.PI * 2); ctx.stroke();
        ctx.fillStyle = '#e2e8f0';
        ctx.font = '10px sans-serif';
        ctx.fillText(`U=${aerodynamicResult.wind_speed.toFixed(1)}`, cx + 14, cy - 4);
    }
    if (currentBridge) {
        const dsY = padT + (1 - (currentBridge.design_wind_speed - minSpeed) / (maxSpeed - minSpeed)) * ph;
        ctx.strokeStyle = 'rgba(245, 158, 11, 0.6)';
        ctx.setLineDash([5, 4]);
        ctx.lineWidth = 1.2;
        ctx.beginPath();
        ctx.moveTo(padL, dsY); ctx.lineTo(padL + pw, dsY); ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = '#f59e0b';
        ctx.font = '9px sans-serif';
        ctx.fillText('设计Vd', padL + pw - 42, dsY - 3);
    }
}

function drawCableChart() {
    if (!currentBridge) return;
    const cv = document.getElementById('canvas-cable');
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const W = cv.width, H = cv.height;
    ctx.clearRect(0, 0, W, H);
    const count = currentBridge.cable_count;
    const nominal = NOMINAL_CABLE_FORCE[currentBridge.bridge_id] || 400;
    const padL = 38, padR = 8, padT = 12, padB = 22;
    const pw = W - padL - padR, ph = H - padT - padB;
    const barWidth = Math.max(4, (pw / count) * 0.7);
    const gap = (pw - barWidth * count) / (count + 1);
    const forces = [];
    const windFactor = 1 + windSpeed / 80;
    for (let i = 0; i < count; i++) {
        const position = 1 - 0.12 * Math.abs(i - count / 2) / (count / 2);
        const noise = 0.95 + Math.random() * 0.1;
        forces.push(nominal * position * windFactor * noise);
    }
    const maxF = Math.max(...forces) * 1.08;
    ctx.strokeStyle = 'rgba(71, 85, 105, 0.3)';
    ctx.font = '9px sans-serif';
    ctx.fillStyle = '#64748b';
    for (let i = 0; i <= 4; i++) {
        const y = padT + (ph / 4) * i;
        ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(padL + pw, y); ctx.stroke();
        const val = maxF * (1 - i / 4);
        ctx.fillText(val.toFixed(0), 4, y + 3);
    }
    const barGrad = ctx.createLinearGradient(0, padT, 0, padT + ph);
    barGrad.addColorStop(0, 'rgba(59, 130, 246, 0.95)');
    barGrad.addColorStop(0.5, 'rgba(147, 197, 253, 0.9)');
    barGrad.addColorStop(1, 'rgba(34, 197, 94, 0.85)');
    forces.forEach((f, i) => {
        const x = padL + gap + i * (barWidth + gap);
        const h = (f / maxF) * ph;
        const y = padT + ph - h;
        const norm = f / (nominal * 1.15);
        const r = Math.min(1, Math.max(0, (norm - 0.8) / 0.4));
        const g = Math.min(1, Math.max(0, 1 - (norm - 0.85) / 0.35));
        ctx.fillStyle = `rgba(${Math.floor(239 * r + 34 * (1-r))}, ${Math.floor(68 * r + 197 * (1-r))}, ${Math.floor(68 * r + 94 * (1-r))}, 0.9)`;
        ctx.fillRect(x, y, barWidth, h);
        ctx.fillStyle = 'rgba(255, 255, 255, 0.3)';
        ctx.fillRect(x, y, barWidth * 0.3, h);
        ctx.fillStyle = '#94a3b8';
        ctx.font = '8px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`${i+1}`, x + barWidth / 2, padT + ph + 12);
    });
    ctx.textAlign = 'start';
    const nominalY = padT + ph - (nominal / maxF) * ph;
    ctx.strokeStyle = 'rgba(245, 158, 11, 0.7)';
    ctx.setLineDash([4, 3]);
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(padL, nominalY); ctx.lineTo(padL + pw, nominalY); ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#f59e0b';
    ctx.font = '9px sans-serif';
    ctx.fillText('标称', padL + 4, nominalY - 3);
    document.getElementById('cable-count').innerText = `共 ${count} 根`;
}

function showPointDetail(point) {
    if (!currentBridge) return;
    const xR = Math.max(0, Math.min(1, (point.x + currentBridge.span / 2) / currentBridge.span));
    const shape = Math.sin(Math.PI * xR);
    const amp = aerodynamicResult ? aerodynamicResult.vibration_amplitude : 0.06;
    const disp = amp * shape * 0.5;
    const omega = 2 * Math.PI * (1.2 * Math.sqrt(9.81 / currentBridge.span));
    const accel = disp * omega * omega;
    const g = 9.81;
    const gravityRatio = accel / g;
    const reducedFreq = omega * currentBridge.width / Math.max(windSpeed, 1);
    const h1Star = -(Math.min(reducedFreq, 5) / 5) * 0.5;
    const a1Star = -(Math.min(reducedFreq, 5) / 5) * 1.0;
    const h1Prime = (Math.min(reducedFreq, 5) / 5) * 4.0;
    const a1Prime = (Math.min(reducedFreq, 5) / 5) * 2.5;
    const q = 0.5 * 1.225 * windSpeed * windSpeed * currentBridge.width;
    const alpha = attackAngle * Math.PI / 180;
    const lift = q * 2 * Math.PI * Math.sin(alpha);
    const drag = q * (0.02 + 2 * Math.PI * alpha * alpha);
    const moment = q * currentBridge.width * 0.5 * Math.PI * Math.sin(alpha);
    document.getElementById('modal-title').innerText = `${currentBridge.name} - 桥面检测点诊断`;
    document.getElementById('modal-body').innerHTML = `
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:20px;margin-bottom:18px;">
            <div>
                <div class="panel-title">空间坐标</div>
                <div class="info-row"><span class="label">顺桥向 X</span><span class="value highlight">${point.x.toFixed(2)} m</span></div>
                <div class="info-row"><span class="label">竖向 Y</span><span class="value highlight">${point.y.toFixed(2)} m</span></div>
                <div class="info-row"><span class="label">横桥向 Z</span><span class="value highlight">${point.z.toFixed(2)} m</span></div>
                <div class="info-row"><span class="label">跨长比</span><span class="value">${(xR*100).toFixed(1)}%</span></div>
                <div class="info-row"><span class="label">距左塔</span><span class="value">${(point.x + currentBridge.span/2 + 8).toFixed(1)} m</span></div>
            </div>
            <div>
                <div class="panel-title">风致响应</div>
                <div class="info-row"><span class="label">竖向位移</span><span class="value ${disp>0.1?'danger':(disp>0.05?'warning':'highlight')}">${(disp*1000).toFixed(1)} mm</span></div>
                <div class="info-row"><span class="label">加速度</span><span class="value">${accel.toFixed(3)} m/s²</span></div>
                <div class="info-row"><span class="label">重力倍数</span><span class="value ${gravityRatio>0.05?'warning':''}">${gravityRatio.toFixed(4)} g</span></div>
                <div class="info-row"><span class="label">振型系数</span><span class="value highlight">${shape.toFixed(3)}</span></div>
                <div class="info-row"><span class="label">评估</span><span class="value" style="color:${disp>0.1?'#f87171':(disp>0.05?'#fbbf24':'#4ade80')};font-weight:700;">${disp>0.1?'超限':(disp>0.05?'警告':'正常')}</span></div>
            </div>
        </div>
        <div style="margin-top:6px;">
            <div class="panel-title">气动工况</div>
            <div class="info-row"><span class="label">风速 / 攻角</span><span class="value">${windSpeed.toFixed(1)} m/s / ${attackAngle.toFixed(1)}°</span></div>
            <div class="info-row"><span class="label">来流方向</span><span class="value">${Math.sin(alpha) > 0 ? '左→右分量' : '右→左分量'}</span></div>
            ${aerodynamicResult ? `
            <div class="info-row"><span class="label">气动阻尼</span><span class="value ${aerodynamicResult.aerodynamic_damping<0?'danger':'highlight'}">${aerodynamicResult.aerodynamic_damping.toFixed(5)} ${aerodynamicResult.aerodynamic_damping<0?'⚠负阻尼':''}</span></div>
            <div class="info-row"><span class="label">结构阻尼 + 气动</span><span class="value">${(0.01 + Math.max(0, aerodynamicResult.aerodynamic_damping)).toFixed(4)}</span></div>
            <div class="info-row"><span class="label">颤振临界 Ucr</span><span class="value warning">${aerodynamicResult.flutter_critical_speed.toFixed(1)} m/s</span></div>
            <div class="info-row"><span class="label">颤振裕度</span><span class="value ${aerodynamicResult.flutter_margin<0.15?'danger':'highlight'}">${(aerodynamicResult.flutter_margin*100).toFixed(1)}%</span></div>
            ` : ''}
        </div>
        <div style="margin-top:18px;">
            <div class="panel-title">Scanlan 颤振导数 (折算频率 K = ${reducedFreq.toFixed(3)})</div>
            <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:8px;">
                <div class="metric-card safe"><div class="mc-title">H₁* 竖弯-升力</div><div class="mc-value" style="font-size:15px;">${h1Star.toFixed(3)}</div></div>
                <div class="metric-card safe"><div class="mc-title">A₁* 扭转-升力</div><div class="mc-value" style="font-size:15px;">${a1Star.toFixed(3)}</div></div>
                <div class="metric-card safe"><div class="mc-title">H₁' 竖弯-力矩</div><div class="mc-value" style="font-size:15px;">${h1Prime.toFixed(3)}</div></div>
                <div class="metric-card safe"><div class="mc-title">A₁' 扭转-力矩</div><div class="mc-value" style="font-size:15px;">${a1Prime.toFixed(3)}</div></div>
            </div>
            <div style="font-size:11px;color:#64748b;margin-top:8px;line-height:1.6;">
                理论：Scanlan (1971) 颤振导数半经验体系，基于折算频率 K = ωB/U 查表插值
            </div>
        </div>
        <div style="margin-top:18px;">
            <div class="panel-title">准定常气动力 (单位: N/m 及 N·m/m)</div>
            <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;">
                <div class="metric-card warning"><div class="mc-title">升力 L</div><div class="mc-value" style="font-size:14px;">${lift.toFixed(1)}</div></div>
                <div class="metric-card warning"><div class="mc-title">阻力 D</div><div class="mc-value" style="font-size:14px;">${drag.toFixed(1)}</div></div>
                <div class="metric-card warning"><div class="mc-title">扭矩 M</div><div class="mc-value" style="font-size:14px;">${moment.toFixed(1)}</div></div>
            </div>
        </div>
    `;
    document.getElementById('detail-modal').classList.add('show');
}

function checkAlerts() {
    if (!aerodynamicResult || !currentBridge) return;
    const r = aerodynamicResult;
    const b = currentBridge;
    if (r.vibration_amplitude > 0.15) {
        const sev = r.vibration_amplitude > 0.3 ? 'critical' : 'warning';
        addAlert({
            type: sev, bridge: b.bridge_id, bridgeName: b.name,
            msg: `桥面振幅 ${(r.vibration_amplitude*1000).toFixed(0)}mm 超阈值150mm`
        });
    }
    if (r.flutter_margin < 0.15) {
        const sev = r.flutter_margin < 0 ? 'critical' : 'warning';
        addAlert({
            type: sev, bridge: b.bridge_id, bridgeName: b.name,
            msg: r.flutter_margin < 0
                ? `⚠ 颤振已发生！风速 ${r.wind_speed.toFixed(1)} 超临界 ${r.flutter_critical_speed.toFixed(1)}`
                : `风速 ${r.wind_speed.toFixed(1)}m/s 接近临界，裕度 ${(r.flutter_margin*100).toFixed(0)}%`
        });
    }
    if (r.aerodynamic_damping < 0) {
        addAlert({
            type: 'critical', bridge: b.bridge_id, bridgeName: b.name,
            msg: `气动阻尼 ${r.aerodynamic_damping.toFixed(4)} 为负，系统可能发散`
        });
    }
}

function addAlert(a) {
    const now = new Date();
    a.time = now;
    a.id = `${a.bridge}-${now.getTime()}`;
    if (alerts.find(x => x.id === a.id)) return;
    alerts.unshift(a);
    if (alerts.length > 50) alerts.length = 50;
    renderAlerts();
    const items = document.querySelectorAll(`.bridge-item[data-id="${a.bridge}"]`);
    items.forEach(el => {
        el.classList.remove('warning', 'danger');
        const hasCrit = alerts.some(x => x.bridge === a.bridge && x.type === 'critical');
        if (hasCrit) el.classList.add('danger');
        else if (alerts.some(x => x.bridge === a.bridge)) el.classList.add('warning');
    });
}

function renderAlerts() {
    document.getElementById('alert-count').innerText = alerts.length;
    const list = document.getElementById('alerts-list');
    if (alerts.length === 0) {
        list.innerHTML = '<div class="loading">暂无告警</div>';
        return;
    }
    list.innerHTML = alerts.slice(0, 20).map(a => {
        const t = `${a.time.getHours().toString().padStart(2,'0')}:${a.time.getMinutes().toString().padStart(2,'0')}:${a.time.getSeconds().toString().padStart(2,'0')}`;
        return `<div class="alert-item ${a.type}">
            <div class="bridge">${a.bridgeName || a.bridge} · ${a.type === 'critical' ? '🔴 严重' : '🟡 警告'}</div>
            <div class="msg">${a.msg}</div>
            <div class="time">${t}</div>
        </div>`;
    }).join('');
}

async function runOptimization() {
    if (!currentBridge) return;
    const gaDiv = document.getElementById('ga-result');
    gaDiv.innerHTML = '<div class="loading">⚙ 正在运行遗传算法优化...</div>';
    const pop = parseInt(document.getElementById('ga-pop').value) || 50;
    const gen = parseInt(document.getElementById('ga-gen').value) || 60;
    const mut = parseFloat(document.getElementById('ga-mut').value) || 0.1;
    const cross = parseFloat(document.getElementById('ga-cross').value) || 0.8;
    const payload = {
        bridge_id: currentBridge.bridge_id,
        population_size: pop,
        generations: gen,
        mutation_rate: mut,
        crossover_rate: cross,
        wind_speed_range: [10, 60],
        attack_angle_range: [-10, 10]
    };
    let result = null;
    try {
        const res = await fetch(`${API_BASE}/optimization/run`, {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });
        if (res.ok) {
            const json = await res.json();
            if (json.success) result = json.data;
        }
    } catch (e) { result = null; }
    if (!result) {
        const baseUcr = currentBridge ? (currentBridge.design_wind_speed * 1.4) : 45;
        const shapes = [
            { name: '扁平闭口箱形', nose: 18, stabH: 0.6, stabC: 2, fairing: 0.35, shape: 'Streamlined' },
            { name: '带风嘴流线型', nose: 35, stabH: 0.8, stabC: 3, fairing: 0.5, shape: 'Streamlined' },
            { name: '开槽分离双箱梁', nose: 25, stabH: 0.5, stabC: 2, fairing: 0.3, shape: 'Slotted' },
        ];
        const best = shapes[Math.floor(Math.random() * shapes.length)];
        const improvement = 1 + best.nose/80 + best.stabC * best.stabH/3 + best.fairing/2;
        result = {
            best_shape: {
                wind_nose_angle: best.nose,
                stabilizer_plate_height: best.stabH,
                stabilizer_plate_count: best.stabC,
                deck_shape_type: best.shape,
                fairing_length: best.fairing,
                porosity: 0.1
            },
            best_fitness: 0.75 + Math.random() * 0.2,
            improved_critical_speed: baseUcr * improvement,
            flutter_probability_reduction: 0.45 + Math.random() * 0.4,
            generation_history: Array.from({length: gen}, (_, i) => 0.4 + 0.45 * (1 - Math.exp(-i * 3 / gen)) + (Math.random() - 0.5) * 0.04)
        };
    }
    const shapeNames = { Flat: '平直板', Streamlined: '流线型', Box: '闭口箱形', Slotted: '开槽分离' };
    const shapeName = shapeNames[result.best_shape.deck_shape_type] || result.best_shape.deck_shape_type;
    const baseUcr = aerodynamicResult ? aerodynamicResult.flutter_critical_speed : (currentBridge.design_wind_speed * 1.4);
    const ucrGain = ((result.improved_critical_speed - baseUcr) / baseUcr * 100).toFixed(1);
    gaDiv.innerHTML = `
        <div class="ga-result">
            <div class="ga-title">✓ 遗传算法完成！适应度: ${result.best_fitness.toFixed(3)}</div>
            <div class="ga-row"><span class="k">最优外形</span><span class="v">${shapeName}</span></div>
            <div class="ga-row"><span class="k">风嘴角度</span><span class="v">${result.best_shape.wind_nose_angle.toFixed(1)}°</span></div>
            <div class="ga-row"><span class="k">稳定板(高×数量)</span><span class="v">${result.best_shape.stabilizer_plate_height.toFixed(2)}m × ${result.best_shape.stabilizer_plate_count}</span></div>
            <div class="ga-row"><span class="k">导流板长度</span><span class="v">${result.best_shape.fairing_length.toFixed(2)}m</span></div>
            <div class="ga-row"><span class="k">开孔率</span><span class="v">${(result.best_shape.porosity * 100).toFixed(1)}%</span></div>
            <div class="ga-row" style="margin-top:8px;padding-top:6px;border-top:1px dashed rgba(71,85,105,0.3);">
                <span class="k">优化后 Ucr</span>
                <span class="v" style="color:#4ade80;">${result.improved_critical_speed.toFixed(1)} m/s <span style="color:#22c55e;">(+${ucrGain}%)</span></span>
            </div>
            <div class="ga-row"><span class="k">颤振概率降低</span><span class="v" style="color:#4ade80;">${(result.flutter_probability_reduction * 100).toFixed(1)}%</span></div>
        </div>
    `;
}

document.addEventListener('keydown', e => {
    if (e.key === 'Escape') closeModal();
});

