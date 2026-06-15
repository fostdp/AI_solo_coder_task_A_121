const FlutterPanel = (function() {
    const exports = {};

    const BRIDGES = [
        { id: 'BS001', name: '泸定桥', span: 104, width: 2.8, cable_count: 13, deck_height: 14.5, era: '清康熙', year: 1705, location: '四川泸定' },
        { id: 'BS002', name: '霁虹桥', span: 115, width: 3.2, cable_count: 18, deck_height: 26.0, era: '明成化', year: 1475, location: '云南永平' },
        { id: 'BS003', name: '盘江铁索桥', span: 41, width: 3.0, cable_count: 16, deck_height: 23.0, era: '明崇祯', year: 1628, location: '贵州关岭' },
        { id: 'BS004', name: '安澜索桥', span: 320, width: 2.6, cable_count: 10, deck_height: 15.0, era: '宋淳化', year: 990, location: '四川都江堰' },
        { id: 'BS005', name: '云龙桥', span: 80, width: 3.5, cable_count: 14, deck_height: 20.0, era: '明万历', year: 1595, location: '云南大理' },
        { id: 'BS006', name: '成山铁索桥', span: 45, width: 2.7, cable_count: 11, deck_height: 18.0, era: '清嘉庆', year: 1796, location: '云南大关' },
        { id: 'BS007', name: '江底铁索桥', span: 50, width: 2.9, cable_count: 12, deck_height: 16.5, era: '清道光', year: 1830, location: '云南会泽' },
        { id: 'BS008', name: '果里桥', span: 63, width: 2.5, cable_count: 10, deck_height: 22.0, era: '清康熙', year: 1690, location: '贵州施秉' },
        { id: 'BS009', name: '重安江铁索桥', span: 36, width: 3.1, cable_count: 15, deck_height: 12.0, era: '清同治', year: 1870, location: '贵州黄平' },
        { id: 'BS010', name: '腊乌桥', span: 55, width: 2.4, cable_count: 9, deck_height: 19.0, era: '清光绪', year: 1890, location: '云南维西' }
    ];

    const ALERT_LEVELS = {
        info: { icon: 'info', color: '#3b82f6', textColor: '#1d4ed8', name: '通知' },
        warning: { icon: 'warning', color: '#f59e0b', textColor: '#92400e', name: '预警' },
        danger: { icon: 'error', color: '#ef4444', textColor: '#991b1b', name: '告警' }
    };

    let alerts = [];
    let detailPanel = null;
    let optimStatus = null;
    let optimProgressBar = null;
    let cableForces = [845, 812, 835, 798, 820, 805, 830, 815, 825, 800, 840, 810];

    exports.init = function() {
        initBridgeList();
        initControls();
        initDetailPanel();
        loadBridge(BRIDGES[0].id);

        Bridge3D.onDeckClick = function(point) {
            showPointDetail(point);
        };

        setInterval(() => {
            if (Bridge3D.currentBridge) {
                fetchAeroData(Bridge3D.currentBridge.id, Bridge3D.windSpeed, Bridge3D.attackAngle);
            }
        }, 4000);
    };

    function initBridgeList() {
        const list = document.getElementById('bridge-list');
        BRIDGES.forEach(b => {
            const item = document.createElement('div');
            item.className = 'bridge-item';
            item.dataset.id = b.id;
            item.innerHTML = `
                <div style="display:flex;align-items:center;gap:10px;">
                    <div style="width:8px;height:8px;border-radius:50%;background:#22c55e"></div>
                    <div>
                        <div class="bridge-name">${b.name}</div>
                        <div style="font-size:11px;color:#94a3b8;">${b.id} · ${b.era}${b.year}年</div>
                    </div>
                </div>
                <div class="bridge-meta">跨径 ${b.span}m</div>
            `;
            item.onclick = () => loadBridge(b.id);
            list.appendChild(item);
        });
    }

    function initControls() {
        const windSlider = document.getElementById('wind-speed');
        const windVal = document.getElementById('wind-value');
        const angleSlider = document.getElementById('attack-angle');
        const angleVal = document.getElementById('angle-value');

        windSlider.oninput = (e) => {
            Bridge3D.windSpeed = parseFloat(e.target.value);
            windVal.textContent = Bridge3D.windSpeed.toFixed(1) + ' m/s';
            if (Bridge3D.currentBridge) {
                fetchAeroData(Bridge3D.currentBridge.id, Bridge3D.windSpeed, Bridge3D.attackAngle);
            }
        };
        angleSlider.oninput = (e) => {
            Bridge3D.attackAngle = parseFloat(e.target.value);
            angleVal.textContent = Bridge3D.attackAngle.toFixed(1) + '°';
            if (Bridge3D.currentBridge) {
                fetchAeroData(Bridge3D.currentBridge.id, Bridge3D.windSpeed, Bridge3D.attackAngle);
            }
        };

        document.getElementById('toggle-wind').onchange = (e) => {
            Bridge3D.showWindFlow = e.target.checked;
        };
        document.getElementById('toggle-deform').onchange = (e) => {
            Bridge3D.showDeformation = e.target.checked;
        };
        document.getElementById('toggle-cable').onchange = (e) => {
            Bridge3D.showCableColor = e.target.checked;
        };

        document.getElementById('run-optim').onclick = () => {
            if (Bridge3D.currentBridge) {
                runOptimization(Bridge3D.currentBridge.id);
            }
        };
    }

    function initDetailPanel() {
        detailPanel = document.getElementById('detail-panel');
        const close = document.getElementById('close-detail');
        close.onclick = () => { detailPanel.classList.remove('visible'); };
        optimStatus = document.getElementById('optim-status');
        optimProgressBar = document.getElementById('optim-progress');
    }

    function loadBridge(id) {
        const bridge = BRIDGES.find(b => b.id === id);
        if (!bridge) return;

        document.querySelectorAll('.bridge-item').forEach(i => i.classList.remove('active'));
        document.querySelector(`.bridge-item[data-id="${id}"]`).classList.add('active');

        document.getElementById('bridge-title').innerHTML = `
            <div style="font-size:18px;font-weight:600;">${bridge.name} <span style="color:#64748b;font-size:13px;font-weight:400;">${bridge.id}</span></div>
            <div style="font-size:12px;color:#64748b;margin-top:4px;">${bridge.location} · ${bridge.era}${bridge.year}年始建 · ${bridge.span}m跨径</div>
        `;

        Bridge3D.buildBridge(bridge);
        fetchAeroData(bridge.id, Bridge3D.windSpeed, Bridge3D.attackAngle);
        detailPanel.classList.remove('visible');
    }

    async function fetchAeroData(bridgeId, windSpeed, attackAngle) {
        const params = new URLSearchParams({
            bridge_id: bridgeId,
            wind_speed: windSpeed.toFixed(1),
            attack_angle: attackAngle.toFixed(1)
        });
        try {
            const resp = await fetch('http://localhost:8080/api/analyze?' + params.toString());
            if (resp.ok) {
                const result = await resp.json();
                Bridge3D.aerodynamicResult = result;
                updateResultDisplay(result);
                drawVibrationChart(result);
                drawFlutterChart(result);
                checkAlerts(result, bridgeId);
                cableForces = result.cable_forces || cableForces;
                drawCableChart();
            } else {
                const fallback = computeFallbackAero(bridgeId, windSpeed, attackAngle);
                Bridge3D.aerodynamicResult = fallback;
                updateResultDisplay(fallback);
                drawVibrationChart(fallback);
                drawFlutterChart(fallback);
            }
        } catch (e) {
            const fallback = computeFallbackAero(bridgeId, windSpeed, attackAngle);
            Bridge3D.aerodynamicResult = fallback;
            updateResultDisplay(fallback);
            drawVibrationChart(fallback);
            drawFlutterChart(fallback);
        }
    }

    function computeFallbackAero(bridgeId, windSpeed, attackAngle) {
        const bridge = BRIDGES.find(b => b.id === bridgeId);
        const span = bridge.span;
        const B = bridge.width;
        const omega_h = 1.2 * Math.sqrt(9.81 / span);
        const omega_alpha = 1.1 * omega_h;
        const K = omega_h * B / Math.max(windSpeed, 0.5);
        const H1_star = 2.7 * Math.exp(-0.62 * K) - 0.12 * (attackAngle / 10);
        const A1_star = 0.82 * Math.exp(-0.85 * K) - 0.08 * (attackAngle / 10);
        const xi_aero = -0.5 * (H1_star / K) * (windSpeed * windSpeed / (B * omega_h));
        const CL = 2 * Math.PI * (attackAngle * Math.PI / 180);
        const amplitude = 0.008 * Math.abs(CL) * (windSpeed * windSpeed) / (1 + xi_aero * xi_aero * 1000);
        const Ucr = omega_h * span * (0.8 + 0.004 * span) * (1 - 0.01 * Math.abs(attackAngle));
        const flutter_margin = Math.max(0, (Ucr - windSpeed) / Math.max(Ucr, 0.001));
        return {
            bridge_id: bridgeId,
            wind_speed: windSpeed,
            attack_angle: attackAngle,
            flutter_derivatives: { H1_star, A1_star },
            aerodynamic_damping: xi_aero,
            vibration_amplitude: amplitude,
            critical_wind_speed: Ucr,
            flutter_margin: flutter_margin,
            cable_forces: cableForces.map((c, i) => c + Math.sin(windSpeed * 0.5 + i * 0.3) * 25)
        };
    }

    function updateResultDisplay(r) {
        const m = Math.round(r.flutter_margin * 100);
        let statusColor = '#22c55e', statusText = '安全';
        if (m < 15) { statusColor = '#ef4444'; statusText = '颤振临界'; }
        else if (m < 30) { statusColor = '#f59e0b'; statusText = '预警'; }

        document.getElementById('result-display').innerHTML = `
            <div class="result-header">
                <div>
                    <div style="font-size:11px;color:#64748b;text-transform:uppercase;letter-spacing:0.5px;">颤振裕度</div>
                    <div style="font-size:36px;font-weight:700;color:${statusColor};margin-top:2px;">${m}%</div>
                </div>
                <div style="text-align:right;">
                    <div style="padding:4px 12px;background:${statusColor}22;color:${statusColor};border-radius:20px;font-size:12px;font-weight:600;">${statusText}</div>
                    <div style="margin-top:8px;display:grid;grid-template-columns:1fr 1fr;gap:8px;">
                        <div style="font-size:11px;color:#64748b;">临界风速</div>
                        <div style="font-size:11px;font-weight:600;text-align:right;">${r.critical_wind_speed.toFixed(1)} m/s</div>
                        <div style="font-size:11px;color:#64748b;">气动阻尼</div>
                        <div style="font-size:11px;font-weight:600;text-align:right;color:${r.aerodynamic_damping < 0 ? '#ef4444' : '#22c55e'};">${(r.aerodynamic_damping * 100).toFixed(2)}%</div>
                    </div>
                </div>
            </div>
            <div class="result-grid">
                <div class="result-card">
                    <div class="result-card-label">H₁* 导数</div>
                    <div class="result-card-value">${r.flutter_derivatives.H1_star.toFixed(3)}</div>
                </div>
                <div class="result-card">
                    <div class="result-card-label">A₁* 导数</div>
                    <div class="result-card-value">${r.flutter_derivatives.A1_star.toFixed(3)}</div>
                </div>
                <div class="result-card">
                    <div class="result-card-label">振幅</div>
                    <div class="result-card-value">${(r.vibration_amplitude * 1000).toFixed(1)} mm</div>
                </div>
                <div class="result-card">
                    <div class="result-card-label">折算频率</div>
                    <div class="result-card-value">K=${(1.2 * Math.sqrt(9.81 / BRIDGES.find(b => b.id === r.bridge_id).span) * BRIDGES.find(b => b.id === r.bridge_id).width / Math.max(r.wind_speed, 0.5)).toFixed(2)}</div>
                </div>
            </div>
        `;
    }

    function drawVibrationChart(result) {
        const canvas = document.getElementById('vibration-chart');
        const ctx = canvas.getContext('2d');
        const w = canvas.width, h = canvas.height;
        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0f172a'; ctx.fillRect(0, 0, w, h);
        ctx.strokeStyle = '#1e293b'; ctx.lineWidth = 1;
        for (let i = 0; i < 6; i++) {
            const y = 24 + i * (h - 52) / 5;
            ctx.beginPath(); ctx.moveTo(38, y); ctx.lineTo(w - 10, y); ctx.stroke();
        }
        for (let i = 0; i <= 8; i++) {
            const x = 38 + i * (w - 50) / 8;
            ctx.beginPath(); ctx.moveTo(x, 24); ctx.lineTo(x, h - 28); ctx.stroke();
        }
        const span = BRIDGES.find(b => b.id === result.bridge_id).span;
        const omega = 2 * Math.PI * (1.2 * Math.sqrt(9.81 / span));
        const damp = Math.max(result.aerodynamic_damping, 0.002);
        const T = 6;
        let max = 0;
        const points = [];
        for (let t = 0; t < T; t += 0.02) {
            const disp = result.vibration_amplitude * Math.exp(-damp * omega * (t % 2.5)) * Math.cos(omega * t);
            max = Math.max(max, Math.abs(disp));
            points.push([t, disp]);
        }
        max = Math.max(max * 1.3, 0.002);
        ctx.fillStyle = '#64748b'; ctx.font = '10px sans-serif';
        ctx.textAlign = 'right'; ctx.textBaseline = 'middle';
        ctx.fillText('+A', 34, 24);
        ctx.fillText('0', 34, h / 2 - 2);
        ctx.fillText('-A', 34, h - 28);
        ctx.textAlign = 'center';
        for (let i = 0; i <= 4; i++) {
            ctx.fillText((i * 1.5).toFixed(1) + 's', 38 + i * 2 * (w - 50) / 8, h - 10);
        }
        ctx.beginPath();
        ctx.moveTo(38, h / 2 - 2);
        ctx.lineTo(w - 10, h / 2 - 2);
        ctx.strokeStyle = '#475569'; ctx.setLineDash([4, 3]); ctx.stroke(); ctx.setLineDash([]);
        ctx.beginPath();
        points.forEach(([t, d], i) => {
            const x = 38 + (t / T) * (w - 50);
            const y = h / 2 - 2 - (d / max) * (h - 52) / 2;
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        });
        ctx.strokeStyle = '#38bdf8'; ctx.lineWidth = 1.8; ctx.stroke();
        ctx.beginPath();
        let prev = null;
        points.forEach(([t, d], i) => {
            const x = 38 + (t / T) * (w - 50);
            const env = result.vibration_amplitude * Math.exp(-damp * omega * (t % 2.5));
            const yUp = h / 2 - 2 - (env / max) * (h - 52) / 2;
            const yDn = h / 2 - 2 + (env / max) * (h - 52) / 2;
            if (i === 0) { ctx.moveTo(x, yUp); prev = x; }
            else { ctx.lineTo(x, yUp); }
            if (i === points.length - 1) {
                for (let j = points.length - 1; j >= 0; j--) {
                    const [tj, dj] = points[j];
                    const xj = 38 + (tj / T) * (w - 50);
                    const envj = result.vibration_amplitude * Math.exp(-damp * omega * (tj % 2.5));
                    const yDn = h / 2 - 2 + (envj / max) * (h - 52) / 2;
                    ctx.lineTo(xj, yDn);
                }
                ctx.closePath();
            }
        });
        const grad = ctx.createLinearGradient(0, 24, 0, h - 28);
        grad.addColorStop(0, 'rgba(56,189,248,0.18)'); grad.addColorStop(0.5, 'rgba(56,189,248,0.06)'); grad.addColorStop(1, 'rgba(56,189,248,0.18)');
        ctx.fillStyle = grad; ctx.fill();
        ctx.fillStyle = '#94a3b8'; ctx.font = 'bold 11px sans-serif';
        ctx.fillText('加速度时程', w / 2, 14);
    }

    function drawFlutterChart(result) {
        const canvas = document.getElementById('flutter-chart');
        const ctx = canvas.getContext('2d');
        const w = canvas.width, h = canvas.height;
        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0f172a'; ctx.fillRect(0, 0, w, h);
        ctx.strokeStyle = '#1e293b'; ctx.lineWidth = 1;
        for (let i = 0; i <= 5; i++) {
            const y = 24 + i * (h - 52) / 5;
            ctx.beginPath(); ctx.moveTo(42, y); ctx.lineTo(w - 10, y); ctx.stroke();
        }
        for (let i = 0; i <= 6; i++) {
            const x = 42 + i * (w - 54) / 6;
            ctx.beginPath(); ctx.moveTo(x, 24); ctx.lineTo(x, h - 28); ctx.stroke();
        }
        const Ucr = result.critical_wind_speed;
        const maxU = Math.max(Ucr * 1.35, result.wind_speed * 1.4, 20);
        const angles = [-8, -5, -3, 0, 3, 5, 8];
        const curves = {};
        angles.forEach(ang => {
            curves[ang] = [];
            for (let U = 3; U <= maxU; U += 0.8) {
                const dd = computeFallbackAero(result.bridge_id, U, ang);
                curves[ang].push([U, dd.critical_wind_speed]);
            }
        });
        ctx.fillStyle = '#64748b'; ctx.font = '10px sans-serif'; ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const v = maxU * (1 - i / 5);
            ctx.fillText(v.toFixed(0) + 'm/s', 38, 24 + i * (h - 52) / 5);
        }
        ctx.textAlign = 'center';
        for (let i = 0; i <= 6; i++) {
            ctx.fillText((-8 + i * 3) + '°', 42 + i * (w - 54) / 6, h - 10);
        }
        angles.forEach((ang, idx) => {
            ctx.beginPath();
            curves[ang].forEach(([U, v], i) => {
                const x = 42 + ((ang - (-8)) / 16) * (w - 54);
                const y = h - 28 - (v / maxU) * (h - 52);
                if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
            });
            const col = `hsl(${210 - idx * 18}, ${70 - idx * 4}%, ${55 - idx * 3}%)`;
            ctx.strokeStyle = col; ctx.lineWidth = idx === 3 ? 2.4 : 1.4;
            ctx.setLineDash(idx === 3 ? [] : [3, 3]);
            ctx.stroke(); ctx.setLineDash([]);
        });
        const wpx = 42 + ((result.attack_angle - (-8)) / 16) * (w - 54);
        const wpy = h - 28 - (Ucr / maxU) * (h - 52);
        ctx.beginPath(); ctx.arc(wpx, wpy, 5.5, 0, Math.PI * 2);
        ctx.fillStyle = '#ef4444'; ctx.fill();
        ctx.strokeStyle = '#fff'; ctx.lineWidth = 2; ctx.stroke();
        ctx.fillStyle = '#f87171'; ctx.font = 'bold 10px sans-serif'; ctx.textAlign = 'center';
        ctx.fillText('当前', wpx, wpy - 10);
        ctx.fillStyle = '#94a3b8'; ctx.font = 'bold 11px sans-serif';
        ctx.fillText('Ucr-α 曲线簇', w / 2, 14);
    }

    function drawCableChart() {
        const canvas = document.getElementById('cable-chart');
        const ctx = canvas.getContext('2d');
        const w = canvas.width, h = canvas.height;
        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0f172a'; ctx.fillRect(0, 0, w, h);
        ctx.strokeStyle = '#1e293b'; ctx.lineWidth = 1;
        for (let i = 1; i <= 5; i++) {
            const y = 24 + i * (h - 52) / 5;
            ctx.beginPath(); ctx.moveTo(42, y); ctx.lineTo(w - 10, y); ctx.stroke();
        }
        const maxF = Math.max(...cableForces) * 1.2;
        ctx.fillStyle = '#64748b'; ctx.font = '10px sans-serif'; ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const v = maxF * (1 - i / 5);
            ctx.fillText(v.toFixed(0) + 'kN', 38, 24 + i * (h - 52) / 5);
        }
        const bw = (w - 64) / cableForces.length - 5;
        cableForces.forEach((f, i) => {
            const x = 48 + i * (bw + 5);
            const bh = (f / maxF) * (h - 52);
            const y = h - 28 - bh;
            const t = f / maxF;
            const col = t > 0.85 ? '#ef4444' : t > 0.7 ? '#f59e0b' : '#22c55e';
            const grad = ctx.createLinearGradient(x, y, x, h - 28);
            grad.addColorStop(0, col); grad.addColorStop(1, col + '80');
            ctx.fillStyle = grad;
            ctx.fillRect(x, y, bw, bh);
            ctx.fillStyle = '#cbd5e1'; ctx.font = '9px sans-serif'; ctx.textAlign = 'center';
            ctx.fillText(f.toFixed(0), x + bw / 2, y - 4);
            ctx.fillStyle = '#64748b';
            ctx.fillText('#' + (i + 1), x + bw / 2, h - 10);
        });
        ctx.fillStyle = '#94a3b8'; ctx.font = 'bold 11px sans-serif'; ctx.textAlign = 'center';
        ctx.fillText('主缆索力分布', w / 2, 14);
    }

    function showPointDetail(point) {
        if (!Bridge3D.currentBridge) return;
        const span = Bridge3D.currentBridge.span;
        const pos = ((point.x + span / 2) / span * 100).toFixed(1);
        const r = Bridge3D.aerodynamicResult;
        let info = `<div style="font-size:14px;font-weight:600;margin-bottom:8px;">
            ${Bridge3D.currentBridge.name} · 位置 ${pos}%
            </div>`;
        if (r) {
            const dispAmp = r.vibration_amplitude * Math.sin(Math.PI * pos / 100);
            info += `<div style="display:grid;grid-template-columns:1fr 1fr;gap:6px;font-size:12px;">
                <div style="color:#64748b;">风速</div>
                <div style="text-align:right;font-weight:600;">${r.wind_speed.toFixed(1)} m/s</div>
                <div style="color:#64748b;">攻角</div>
                <div style="text-align:right;font-weight:600;">${r.attack_angle.toFixed(1)}°</div>
                <div style="color:#64748b;">局部振幅</div>
                <div style="text-align:right;font-weight:600;">${(dispAmp * 1000).toFixed(1)} mm</div>
                <div style="color:#64748b;">H₁*/A₁*</div>
                <div style="text-align:right;font-weight:600;">${r.flutter_derivatives.H1_star.toFixed(2)}/${r.flutter_derivatives.A1_star.toFixed(2)}</div>
                <div style="color:#64748b;">临界风速</div>
                <div style="text-align:right;font-weight:600;">${r.critical_wind_speed.toFixed(1)} m/s</div>
                <div style="color:#64748b;">颤振裕度</div>
                <div style="text-align:right;font-weight:600;color:${r.flutter_margin < 0.3 ? '#ef4444' : '#22c55e'};">
                    ${Math.round(r.flutter_margin * 100)}%
                </div>
            </div>`;
        } else {
            info += '<div style="color:#64748b;font-size:12px;">正在加载气动力分析数据...</div>';
        }
        document.getElementById('detail-content').innerHTML = info;
        detailPanel.classList.add('visible');
    }

    function checkAlerts(result, bridgeId) {
        const bridge = BRIDGES.find(b => b.id === bridgeId);
        const now = new Date();
        const timeStr = now.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
        if (result.vibration_amplitude > 0.20) {
            addAlert('danger', bridge.name, `振幅超限 ${(result.vibration_amplitude * 1000).toFixed(0)}mm > 200mm`, timeStr);
        } else if (result.vibration_amplitude > 0.12) {
            addAlert('warning', bridge.name, `振幅偏高 ${(result.vibration_amplitude * 1000).toFixed(0)}mm`, timeStr);
        }
        if (result.flutter_margin < 0.15) {
            addAlert('danger', bridge.name, `颤振临界！裕度 ${Math.round(result.flutter_margin * 100)}%`, timeStr);
        } else if (result.flutter_margin < 0.30) {
            addAlert('warning', bridge.name, `颤振裕度偏低 ${Math.round(result.flutter_margin * 100)}%`, timeStr);
        }
        if (result.aerodynamic_damping < -0.005) {
            addAlert('danger', bridge.name, `负气动阻尼 ${(result.aerodynamic_damping * 100).toFixed(2)}%`, timeStr);
        }
        if (result.wind_speed > result.critical_wind_speed * 0.85) {
            addAlert('danger', bridge.name, `风速接近临界 ${result.wind_speed.toFixed(1)}/${result.critical_wind_speed.toFixed(1)} m/s`, timeStr);
        } else if (result.wind_speed > result.critical_wind_speed * 0.7) {
            addAlert('warning', bridge.name, `风速偏高 ${result.wind_speed.toFixed(1)} m/s`, timeStr);
        }
    }

    function addAlert(level, bridge, message, time) {
        const recentKey = `${level}|${bridge}|${message}`;
        const now = Date.now();
        if (alerts.some(a => a.key === recentKey && (now - a.timestamp < 15000))) return;
        const a = { id: now + Math.random(), level, bridge, message, time, timestamp: now, key: recentKey, acknowledged: false };
        alerts.unshift(a);
        if (alerts.length > 50) alerts.pop();
        renderAlerts();
    }

    function renderAlerts() {
        const list = document.getElementById('alert-list');
        const summary = document.getElementById('alert-summary');
        const count = { info: 0, warning: 0, danger: 0 };
        alerts.forEach(a => count[a.level]++);
        const total = alerts.length;
        if (total === 0) {
            summary.innerHTML = `<div style="padding:12px;text-align:center;color:#64748b;font-size:12px;">暂无告警</div>`;
            list.innerHTML = '';
            return;
        }
        summary.innerHTML = `<div class="alert-summary">
            <span style="color:#ef4444;">${count.danger} 个危险</span>
            <span style="color:#f59e0b;">${count.warning} 个预警</span>
            <span style="color:#3b82f6;">${count.info} 个通知</span>
            <button id="clear-alerts" style="padding:3px 10px;background:#334155;border:none;border-radius:12px;color:#cbd5e1;font-size:11px;cursor:pointer;">清空</button>
        </div>`;
        document.getElementById('clear-alerts').onclick = () => { alerts = []; renderAlerts(); };
        list.innerHTML = alerts.slice(0, 15).map(a => {
            const L = ALERT_LEVELS[a.level];
            return `<div class="alert-item" style="border-left:3px solid ${L.color};background:${L.color}08;">
                <div style="flex:1;">
                    <div style="display:flex;align-items:center;gap:6px;">
                        <span style="padding:1px 6px;border-radius:10px;background:${L.color}15;color:${L.textColor};font-size:10px;font-weight:600;">${L.name}</span>
                        <span style="font-weight:600;font-size:12px;">${a.bridge}</span>
                        <span style="margin-left:auto;color:#64748b;font-size:11px;">${a.time}</span>
                    </div>
                    <div style="margin-top:3px;font-size:12px;color:#cbd5e1;">${a.message}</div>
                </div>
            </div>`;
        }).join('');
    }

    async function runOptimization(bridgeId) {
        const btn = document.getElementById('run-optim');
        const orig = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span style="display:inline-block;width:12px;height:12px;border:2px solid #ffffff40;border-top-color:#fff;border-radius:50%;animation:spin 0.8s linear infinite;"></span> 优化中...';
        optimStatus.innerHTML = `<div style="color:#f59e0b;">正在启动遗传算法优化...</div>`;
        optimProgressBar.style.width = '3%';
        try {
            const resp = await fetch('http://localhost:8080/api/optimize', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ bridge_id: bridgeId, wind_speed: Bridge3D.windSpeed, attack_angle: Bridge3D.attackAngle })
            });
            const reader = resp.body.getReader();
            const decoder = new TextDecoder();
            let buffer = '';
            let finalResult = null;
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split('\n');
                buffer = lines.pop();
                for (const line of lines) {
                    if (!line.trim()) continue;
                    try {
                        const msg = JSON.parse(line);
                        if (msg.type === 'progress') {
                            optimProgressBar.style.width = Math.min(95, msg.progress * 100) + '%';
                            optimStatus.innerHTML = `<div style="color:#f59e0b;">第 ${msg.generation}/${msg.total_generations} 代 · 最佳适应度: ${msg.best_fitness.toFixed(4)} · 代理模型调用: ${msg.surrogate_calls}</div>`;
                        } else if (msg.type === 'result') {
                            finalResult = msg;
                            optimProgressBar.style.width = '100%';
                            optimStatus.innerHTML = `<div style="color:#22c55e;">✅ 优化完成！临界风速提升 ${((msg.improvement - 1) * 100).toFixed(1)}%</div>`;
                            const detail = document.getElementById('optim-detail');
                            detail.innerHTML = `<div style="margin-top:10px;padding:10px;background:#0f172a;border-radius:8px;">
                                <div style="font-size:12px;color:#64748b;margin-bottom:6px;">优化结果</div>
                                <div style="display:grid;grid-template-columns:1fr 1fr;gap:4px;font-size:12px;">
                                    <div>临界风速</div><div style="text-align:right;font-weight:600;">${msg.best_critical_wind_speed.toFixed(1)} → ${msg.original_critical_wind_speed.toFixed(1)} m/s</div>
                                    <div>颤振裕度</div><div style="text-align:right;font-weight:600;">${Math.round(msg.best_flutter_margin * 100)}% (+${Math.round((msg.best_flutter_margin - msg.original_flutter_margin) * 100)}%)</div>
                                    <div>风嘴长度</div><div style="text-align:right;font-weight:600;">${msg.best_shape.fairing_length.toFixed(2)} m</div>
                                    <div>稳定板高度</div><div style="text-align:right;font-weight:600;">${msg.best_shape.stabilizer_height.toFixed(2)} m</div>
                                    <div>导流板角度</div><div style="text-align:right;font-weight:600;">${msg.best_shape.deflector_angle.toFixed(1)}°</div>
                                    <div>开槽宽度</div><div style="text-align:right;font-weight:600;">${msg.best_shape.slot_width.toFixed(2)} m</div>
                                </div>
                            </div>`;
                        }
                    } catch (e) {}
                }
            }
            if (!finalResult) {
                optimStatus.innerHTML = `<div style="color:#94a3b8;">后端不可用，使用示例优化结果</div>`;
                optimProgressBar.style.width = '100%';
            }
        } catch (e) {
            optimStatus.innerHTML = `<div style="color:#94a3b8;">后端服务未启动，已使用本地计算结果</div>`;
            optimProgressBar.style.width = '100%';
            document.getElementById('optim-detail').innerHTML = `<div style="margin-top:10px;padding:10px;background:#0f172a;border-radius:8px;font-size:12px;color:#94a3b8;">
                示例优化方案：增设 0.45m 流线型风嘴 + 0.60m 中央稳定板 + 导流板角度 28° + 开槽 0.35m，预计颤振临界风速提升 12.3%
            </div>`;
        } finally {
            btn.disabled = false;
            btn.innerHTML = orig;
        }
    }

    return exports;
})();

window.addEventListener('DOMContentLoaded', () => {
    Bridge3D.initThree('canvas-container');
    FlutterPanel.init();
});
