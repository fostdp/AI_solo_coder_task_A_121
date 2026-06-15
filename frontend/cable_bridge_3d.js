const Bridge3D = (function() {
    const exports = {};

    exports.scene = null;
    exports.camera = null;
    exports.renderer = null;
    exports.controls = null;
    exports.raycaster = null;
    exports.mouse = null;
    exports.deckMesh = null;
    exports.cableMeshes = [];
    exports.towerMeshes = [];
    exports.windParticles = null;
    exports.timeStart = Date.now();
    exports.currentBridge = null;
    exports.windSpeed = 15;
    exports.attackAngle = 0;
    exports.showWindFlow = true;
    exports.showDeformation = true;
    exports.showCableColor = false;
    exports.aerodynamicResult = null;
    exports.onDeckClick = null;

    const WIND_VERTEX_SHADER = `
    attribute vec3 aSeed;
    attribute vec3 aBaseVel;
    attribute vec3 aBaseColor;
    attribute float aIndex;
    uniform float uTime;
    uniform float uWindSpeed;
    uniform float uAttackAngle;
    uniform float uSpan;
    uniform float uWidth;
    uniform float uDeckHeight;
    varying vec3 vColor;
    varying float vAlpha;
    float hash(float n) { return fract(sin(n) * 43758.5453); }
    void main() {
        float sf = (uWindSpeed / 15.0) * (1.0 + uAttackAngle / 60.0);
        float dirOffset = (uAttackAngle / 180.0) * 3.14159265;
        float streamX = mod(aSeed.x + aBaseVel.x * uTime * sf * 0.85, uSpan * 2.9) - uSpan * 1.45;
        float recyclePhase = floor((aSeed.x + aBaseVel.x * uTime * sf * 0.85) / (uSpan * 2.9));
        float seedY = aSeed.y + hash(recyclePhase + aIndex) * uSpan * 0.45;
        float seedZ = aSeed.z + hash(recyclePhase + aIndex * 1.7) * uWidth * 18.0 - uWidth * 9.0;
        float verticalShear = 1.0 + ((seedY - uDeckHeight) / (uSpan * 0.4)) * 0.4;
        verticalShear = max(0.4, verticalShear);
        float posX = streamX * verticalShear;
        float posY = seedY + aBaseVel.y * uTime * 0.06;
        float swirl = sin(uTime * 0.8 + aIndex * 0.05) * 0.025 * sf;
        float posZ = seedZ + aBaseVel.z * uTime * 0.06 + swirl + sin(dirOffset) * 0.04 * sf;
        posZ = clamp(posZ, -uWidth * 10.0, uWidth * 10.0);
        vec3 worldPos = vec3(posX, posY, posZ);
        float t = clamp((seedY - (uDeckHeight - 5.0)) / (uSpan * 0.45), 0.0, 1.0);
        vColor = aBaseColor * (0.9 + 0.2 * hash(aIndex + uTime * 0.3));
        vAlpha = 0.65 + 0.3 * sin(uTime * 2.0 + aIndex * 0.1);
        vec4 mvPosition = modelViewMatrix * vec4(worldPos, 1.0);
        gl_PointSize = 1.35 * (300.0 / -mvPosition.z) * (0.8 + 0.4 * verticalShear);
        gl_Position = projectionMatrix * mvPosition;
    }`;

    const WIND_FRAGMENT_SHADER = `
    varying vec3 vColor;
    varying float vAlpha;
    void main() {
        vec2 uv = gl_PointCoord - 0.5;
        float d = length(uv);
        if (d > 0.5) discard;
        float glow = smoothstep(0.5, 0.0, d);
        gl_FragColor = vec4(vColor, glow * vAlpha * 0.78);
    }`;

    const cB = new THREE.Color(0x2266cc), cG = new THREE.Color(0x22c55e);
    const cY = new THREE.Color(0xeab308), cO = new THREE.Color(0xf97316), cR = new THREE.Color(0xdc2626);
    const workColor = new THREE.Color();

    function easeInOutCubic(t) { return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2; }

    exports.initThree = function(containerId) {
        const container = document.getElementById(containerId);
        exports.scene = new THREE.Scene();
        exports.scene.fog = new THREE.FogExp2(0x0a0e1a, 0.006);
        exports.camera = new THREE.PerspectiveCamera(55, container.clientWidth / container.clientHeight, 0.1, 5000);
        exports.camera.position.set(120, 60, 100);
        exports.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
        exports.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        exports.renderer.setSize(container.clientWidth, container.clientHeight);
        exports.renderer.shadowMap.enabled = true;
        exports.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
        container.appendChild(exports.renderer.domElement);
        exports.controls = new THREE.OrbitControls(exports.camera, exports.renderer.domElement);
        exports.controls.enableDamping = true;
        exports.controls.dampingFactor = 0.06;
        exports.controls.minDistance = 20;
        exports.controls.maxDistance = 500;
        exports.controls.maxPolarAngle = Math.PI * 0.92;
        exports.scene.add(new THREE.AmbientLight(0x6080a0, 0.45));
        const dirLight = new THREE.DirectionalLight(0xfff0d8, 0.95);
        dirLight.position.set(120, 180, 100);
        dirLight.castShadow = true;
        dirLight.shadow.mapSize.set(2048, 2048);
        dirLight.shadow.camera.left = -250; dirLight.shadow.camera.right = 250;
        dirLight.shadow.camera.top = 250; dirLight.shadow.camera.bottom = -250;
        dirLight.shadow.camera.near = 0.5; dirLight.shadow.camera.far = 600;
        exports.scene.add(dirLight);
        const rimLight = new THREE.DirectionalLight(0x5a7a9a, 0.35);
        rimLight.position.set(-80, 60, -100);
        exports.scene.add(rimLight);
        exports.scene.add(new THREE.HemisphereLight(0x87a9c8, 0x2a3a4a, 0.35));
        createEnvironment();
        exports.raycaster = new THREE.Raycaster();
        exports.mouse = new THREE.Vector2();
        exports.renderer.domElement.addEventListener('click', onCanvasClick);
        exports.renderer.domElement.addEventListener('mousemove', onCanvasHover);
        window.addEventListener('resize', onWindowResize);
        animate();
    };

    function createEnvironment() {
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
        exports.scene.add(water);

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
            exports.scene.add(m);
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
            exports.scene.add(trunk);
            const lh = 5 + Math.random() * 5;
            const leaves = new THREE.Mesh(new THREE.ConeGeometry(2.5 + Math.random() * 2.5, lh, 6), leafMats[i % 2]);
            leaves.position.set(tx, th + lh / 2 - 1.5, tz);
            exports.scene.add(leaves);
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
        exports.scene.add(new THREE.Points(starsGeo, new THREE.PointsMaterial({
            color: 0xeef5ff, size: 1.3, sizeAttenuation: false, transparent: true, opacity: 0.75
        })));
    }

    exports.buildBridge = function(bridge) {
        exports.currentBridge = bridge;
        exports.cableMeshes.forEach(m => exports.scene.remove(m));
        exports.towerMeshes.forEach(m => exports.scene.remove(m));
        if (exports.deckMesh) exports.scene.remove(exports.deckMesh);
        if (exports.windParticles) exports.scene.remove(exports.windParticles);
        exports.cableMeshes = []; exports.towerMeshes = []; exports.deckMesh = null; exports.windParticles = null;

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
            exports.scene.add(base); exports.towerMeshes.push(base);
            const pillar = new THREE.Mesh(new THREE.BoxGeometry(3.2, towerHeight, 4.2), stoneMat);
            pillar.position.set(tx, towerHeight / 2 - 2, 0); pillar.castShadow = true; pillar.receiveShadow = true;
            exports.scene.add(pillar); exports.towerMeshes.push(pillar);
            const cap = new THREE.Mesh(new THREE.BoxGeometry(4.8, 2.8, 5.8), stoneDark);
            cap.position.set(tx, towerHeight + 0.6 - 2, 0); cap.castShadow = true;
            exports.scene.add(cap); exports.towerMeshes.push(cap);
            for (let col = -1; col <= 1; col += 2) {
                for (let r = 0; r < 2; r++) {
                    const rope = new THREE.Mesh(
                        new THREE.CylinderGeometry(0.09, 0.09, towerHeight * 0.95, 6),
                        new THREE.MeshStandardMaterial({ color: 0x6a5a4a, roughness: 0.85 })
                    );
                    rope.position.set(tx + col * 1.1 + r * 0.1, towerHeight * 0.45 - 2, col * 1.5);
                    exports.scene.add(rope); exports.towerMeshes.push(rope);
                }
            }
            const anchor = new THREE.Mesh(new THREE.BoxGeometry(9, 6, 11), stoneDark);
            anchor.position.set(side * (span / 2 + 13), 1, 0); anchor.castShadow = true; anchor.receiveShadow = true;
            exports.scene.add(anchor); exports.towerMeshes.push(anchor);
            const anchorTop = new THREE.Mesh(new THREE.BoxGeometry(7, 1.5, 9), stoneMat);
            anchorTop.position.set(side * (span / 2 + 13), 4.75, 0);
            exports.scene.add(anchorTop); exports.towerMeshes.push(anchorTop);
        });

        const deckSegments = 60;
        const deckGeo = new THREE.BoxGeometry(span, 0.35, width, deckSegments, 1, 5);
        const deckMat = new THREE.MeshStandardMaterial({
            color: 0x8a6a40, roughness: 0.82, metalness: 0.08, vertexColors: true
        });
        exports.deckMesh = new THREE.Mesh(deckGeo, deckMat);
        exports.deckMesh.position.y = deckHeight;
        exports.deckMesh.castShadow = true; exports.deckMesh.receiveShadow = true;
        exports.deckMesh.userData.originalPositions = [];
        const dp = deckGeo.attributes.position;
        const colors = new Float32Array(dp.count * 3);
        for (let i = 0; i < dp.count; i++) {
            exports.deckMesh.userData.originalPositions.push(dp.getX(i), dp.getY(i), dp.getZ(i));
            const tone = 0.9 + Math.random() * 0.2;
            colors[i*3] = 0.55 * tone; colors[i*3+1] = 0.42 * tone; colors[i*3+2] = 0.25 * tone;
        }
        deckGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
        exports.scene.add(exports.deckMesh);

        const frameMat = new THREE.MeshStandardMaterial({ color: 0x3a2a1a, roughness: 0.75 });
        for (let s = 0; s <= deckSegments; s += 2) {
            const x = -span / 2 + (s / deckSegments) * span;
            const cb = new THREE.Mesh(new THREE.BoxGeometry(0.18, 0.22, width + 0.6), frameMat);
            cb.position.set(x, deckHeight - 0.06, 0); exports.deckMesh.add(cb);
        }
        const sideBeam = new THREE.Mesh(new THREE.BoxGeometry(span, 0.15, 0.18), frameMat);
        sideBeam.position.set(0, deckHeight - 0.05, width / 2 + 0.09); exports.deckMesh.add(sideBeam);
        const sideBeam2 = sideBeam.clone(); sideBeam2.position.z = -width / 2 - 0.09; exports.deckMesh.add(sideBeam2);

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
            exports.cableMeshes.push(mesh); exports.scene.add(mesh);
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
                exports.scene.add(hang); exports.cableMeshes.push(hang);
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
                    post.position.set(px, deckHeight + 0.7, zBase); exports.deckMesh.add(post);
                }
                const railTop = new THREE.Mesh(new THREE.CylinderGeometry(0.025, 0.025, span, 5), railMat);
                railTop.rotation.z = Math.PI / 2; railTop.position.set(0, deckHeight + 1.18, zBase); exports.deckMesh.add(railTop);
                const railMid = railTop.clone(); railMid.position.y = deckHeight + 0.7; exports.deckMesh.add(railMid);
            }
        }
        createWindParticles(span, width, deckHeight);
        exports.updateCameraForBridge(bridge);
    };

    exports.updateCameraForBridge = function(bridge) {
        const dist = bridge.span * 1.25;
        exports.camera.position.set(dist * 0.65, dist * 0.52, dist * 1.0);
        exports.controls.target.set(0, bridge.deck_height + bridge.span * 0.04, 0); exports.controls.update();
    };

    function createWindParticles(span, width, deckHeight) {
        const count = 1500;
        const baseGeo = new THREE.BufferGeometry();
        const dummyPos = new Float32Array([0, 0, 0]);
        baseGeo.setAttribute('position', new THREE.BufferAttribute(dummyPos, 3));
        const seeds = new Float32Array(count * 3);
        const baseVels = new Float32Array(count * 3);
        const baseColors = new Float32Array(count * 3);
        const indices = new Float32Array(count);
        for (let i = 0; i < count; i++) {
            seeds[i*3] = (Math.random() - 0.5) * span * 2.8;
            seeds[i*3+1] = deckHeight - 5 + (Math.random() - 0.2) * span * 0.45;
            seeds[i*3+2] = (Math.random() - 0.5) * width * 18;
            baseVels[i*3] = 0.8 + Math.random() * 2.2;
            baseVels[i*3+1] = (Math.random() - 0.5) * 0.35;
            baseVels[i*3+2] = (Math.random() - 0.5) * 0.25;
            const t = ((seeds[i*3+1] - (deckHeight - 5)) / (span * 0.45));
            const hue = 0.54 + t * 0.12, sat = 0.72, val = 0.52 + t * 0.18;
            const c = new THREE.Color().setHSL(hue, sat, val);
            baseColors[i*3] = c.r; baseColors[i*3+1] = c.g; baseColors[i*3+2] = c.b;
            indices[i] = i;
        }
        const instancedGeo = new THREE.InstancedBufferGeometry();
        instancedGeo.index = baseGeo.index;
        instancedGeo.setAttribute('position', baseGeo.getAttribute('position'));
        instancedGeo.setAttribute('aSeed', new THREE.InstancedBufferAttribute(seeds, 3));
        instancedGeo.setAttribute('aBaseVel', new THREE.InstancedBufferAttribute(baseVels, 3));
        instancedGeo.setAttribute('aBaseColor', new THREE.InstancedBufferAttribute(baseColors, 3));
        instancedGeo.setAttribute('aIndex', new THREE.InstancedBufferAttribute(indices, 1));
        instancedGeo.instanceCount = count;
        const windUniforms = {
            uTime: { value: 0 },
            uWindSpeed: { value: 12 },
            uAttackAngle: { value: 3 },
            uSpan: { value: span },
            uWidth: { value: width },
            uDeckHeight: { value: deckHeight },
        };
        exports.windParticles = new THREE.Points(instancedGeo, new THREE.ShaderMaterial({
            vertexShader: WIND_VERTEX_SHADER,
            fragmentShader: WIND_FRAGMENT_SHADER,
            uniforms: windUniforms,
            transparent: true,
            blending: THREE.AdditiveBlending,
            depthWrite: false,
        }));
        exports.windParticles.userData = { uniforms: windUniforms, span, width, deckHeight, isGPUParticles: true };
        exports.scene.add(exports.windParticles);
    }

    function onCanvasHover(e) {
        if (!exports.deckMesh) return;
        const rect = exports.renderer.domElement.getBoundingClientRect();
        exports.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
        exports.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
        exports.raycaster.setFromCamera(exports.mouse, exports.camera);
        const hits = exports.raycaster.intersectObject(exports.deckMesh, true);
        exports.renderer.domElement.style.cursor = hits.length > 0 ? 'pointer' : 'default';
    }

    function onCanvasClick(event) {
        if (!exports.deckMesh || !exports.currentBridge) return;
        const rect = exports.renderer.domElement.getBoundingClientRect();
        exports.mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        exports.mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
        exports.raycaster.setFromCamera(exports.mouse, exports.camera);
        const hits = exports.raycaster.intersectObject(exports.deckMesh, true);
        if (hits.length > 0 && exports.onDeckClick) {
            exports.onDeckClick(hits[0].point);
        }
    }

    function onWindowResize() {
        const container = document.getElementById('canvas-container');
        exports.camera.aspect = container.clientWidth / container.clientHeight;
        exports.camera.updateProjectionMatrix();
        exports.renderer.setSize(container.clientWidth, container.clientHeight);
    }

    function animate() {
        requestAnimationFrame(animate);
        exports.controls.update();
        const now = Date.now();
        const elapsed = (now - exports.timeStart) / 1000;

        if (exports.windParticles && exports.showWindFlow) {
            exports.windParticles.visible = true;
            if (exports.windParticles.userData.isGPUParticles) {
                const u = exports.windParticles.userData.uniforms;
                u.uTime.value = elapsed;
                u.uWindSpeed.value = exports.windSpeed;
                u.uAttackAngle.value = exports.attackAngle;
            }
        } else if (exports.windParticles) {
            exports.windParticles.visible = false;
        }

        exports.scene.children.forEach(obj => {
            if (obj.userData && obj.userData.isWater && obj.userData.waterPositions) {
                const wp = obj.userData.waterPositions;
                const orig = obj.userData.originalZ;
                for (let i = 0; i < wp.count; i++) {
                    const ox = wp.getX(i);
                    const oy = wp.getY(i);
                    const extra = Math.sin(ox * 0.012 + elapsed * 0.7 + oy * 0.01) * 0.15
                                + Math.sin(ox * 0.035 + elapsed * 1.4) * 0.08;
                    wp.setZ(i, orig[i] + extra * (0.6 + exports.windSpeed / 50));
                }
                wp.needsUpdate = true;
                obj.geometry.computeVertexNormals();
            }
        });

        if (exports.deckMesh && exports.currentBridge) {
            const span = exports.currentBridge.span;
            const amp = exports.aerodynamicResult ? exports.aerodynamicResult.vibration_amplitude : (exports.windSpeed / 35) * 0.16;
            const omega = 2 * Math.PI * (1.2 * Math.sqrt(9.81 / span));
            const damp = exports.aerodynamicResult ? Math.max(exports.aerodynamicResult.aerodynamic_damping, 0.001) : 0.012;
            const t = elapsed % 6;
            const envelope = Math.exp(-damp * omega * (t % 2.5));
            const phaseT = omega * t;
            const pos = exports.deckMesh.geometry.attributes.position;
            const colors = exports.deckMesh.geometry.attributes.color.array;
            const orig = exports.deckMesh.userData.originalPositions;
            for (let i = 0; i < pos.count; i++) {
                const ox = orig[i*3], oy = orig[i*3+1], oz = orig[i*3+2];
                const xR = (ox + span / 2) / span;
                const shape = Math.sin(Math.PI * xR);
                const dispStatic = amp * shape * 0.45;
                const dispVib = amp * shape * 0.55 * envelope * Math.cos(phaseT + xR * Math.PI);
                const totalDisp = dispStatic + dispVib;
                const torsion = 0.004 * totalDisp * (exports.attackAngle / 10) * (exports.windSpeed / 30) * (oz / exports.currentBridge.width);
                pos.setXYZ(i, ox, oy + totalDisp + torsion, oz);
                if (exports.showDeformation) {
                    const nd = Math.min(Math.abs(totalDisp) / 0.22, 1.0);
                    let c;
                    if (nd < 0.25) c = workColor.copy(cB).lerp(cG, nd / 0.25);
                    else if (nd < 0.5) c = workColor.copy(cG).lerp(cY, (nd - 0.25) / 0.25);
                    else if (nd < 0.75) c = workColor.copy(cY).lerp(cO, (nd - 0.5) / 0.25);
                    else c = workColor.copy(cO).lerp(cR, (nd - 0.75) / 0.25);
                    const tone = 0.92 + Math.sin(i * 0.1) * 0.08;
                    colors[i*3] = c.r * tone; colors[i*3+1] = c.g * tone; colors[i*3+2] = c.b * tone;
                } else {
                    const tone = 0.9 + Math.sin(i * 0.1) * 0.1;
                    colors[i*3] = 0.55 * tone; colors[i*3+1] = 0.42 * tone; colors[i*3+2] = 0.25 * tone;
                }
            }
            pos.needsUpdate = true;
            exports.deckMesh.geometry.attributes.color.needsUpdate = true;
            exports.deckMesh.geometry.computeVertexNormals();
        }

        if (exports.showCableColor && exports.cableMeshes.length) {
            exports.cableMeshes.forEach((m, i) => {
                if (m.userData.baseColor) {
                    const ratio = Math.min(exports.windSpeed / 40, 1) * (0.7 + 0.3 * Math.sin(elapsed * 1.5 + i * 0.6));
                    m.material.color.copy(m.userData.baseColor).lerp(workColor.set(0xef4444), ratio * 0.55);
                }
            });
        }

        exports.renderer.render(exports.scene, exports.camera);
    }

    return exports;
})();
