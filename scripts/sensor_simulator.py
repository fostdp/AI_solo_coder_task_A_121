#!/usr/bin/env python3
"""
古代悬索桥风致振动监测系统 - 传感器模拟器
模拟10座古代铁索桥通过4G DTU每10分钟上报数据
"""

import json
import random
import time
import threading
import http.client
import urllib.parse
from datetime import datetime, timezone
from dataclasses import dataclass, asdict
from typing import List, Dict, Optional

BRIDGES = [
    {"bridge_id": "BS001", "name": "泸定桥", "cable_count": 13, "span": 100.0, "design_wind_speed": 35.0},
    {"bridge_id": "BS002", "name": "霁虹桥", "cable_count": 18, "span": 106.0, "design_wind_speed": 32.0},
    {"bridge_id": "BS003", "name": "云龙桥", "cable_count": 12, "span": 88.0, "design_wind_speed": 30.0},
    {"bridge_id": "BS004", "name": "重安江铁索桥", "cable_count": 15, "span": 36.5, "design_wind_speed": 28.0},
    {"bridge_id": "BS005", "name": "盘江铁索桥", "cable_count": 14, "span": 71.0, "design_wind_speed": 38.0},
    {"bridge_id": "BS006", "name": "程阳桥", "cable_count": 10, "span": 58.0, "design_wind_speed": 25.0},
    {"bridge_id": "BS007", "name": "金龙桥", "cable_count": 16, "span": 108.0, "design_wind_speed": 40.0},
    {"bridge_id": "BS008", "name": "豆沙关铁索桥", "cable_count": 11, "span": 49.0, "design_wind_speed": 33.0},
    {"bridge_id": "BS009", "name": "普安桥", "cable_count": 9, "span": 42.0, "design_wind_speed": 29.0},
    {"bridge_id": "BS010", "name": "安顺场铁索桥", "cable_count": 12, "span": 62.0, "design_wind_speed": 31.0},
]

BASE_NOMINAL_FORCE = {
    "BS001": 520000, "BS002": 480000, "BS003": 410000, "BS004": 380000, "BS005": 460000,
    "BS006": 350000, "BS007": 550000, "BS008": 390000, "BS009": 370000, "BS010": 400000,
}

ACCELERATION_SENSOR_POSITIONS = [0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9]

@dataclass
class CableForceReading:
    cable_id: str
    force: float
    temperature: float

@dataclass
class AccelerationReading:
    sensor_id: str
    position_x: float
    ax: float
    ay: float
    az: float

@dataclass
class WindReading:
    sensor_id: str
    speed: float
    direction: float
    attack_angle: float
    temperature: float
    humidity: float

@dataclass
class DTUPayload:
    device_id: str
    bridge_id: str
    timestamp: str
    cable_forces: List[CableForceReading]
    accelerations: List[AccelerationReading]
    wind: WindReading

class BridgeSensorSimulator:
    def __init__(self, api_host: str = "localhost", api_port: int = 8080, interval_seconds: int = 600):
        self.api_host = api_host
        self.api_port = api_port
        self.interval = interval_seconds
        self.stop_event = threading.Event()
        self.threads = []
        self.wind_base = {b["bridge_id"]: random.uniform(3.0, 12.0) for b in BRIDGES}
        self.wind_gust_factor = {b["bridge_id"]: random.uniform(0.0, 1.0) for b in BRIDGES}
        self.cable_force_offset = {}
        for b in BRIDGES:
            self.cable_force_offset[b["bridge_id"]] = {
                f"C{i+1:02d}": random.uniform(-0.02, 0.02) for i in range(b["cable_count"])
            }

    def generate_wind(self, bridge_info: Dict, timestamp: datetime) -> WindReading:
        bid = bridge_info["bridge_id"]
        base = self.wind_base[bid]
        hour = timestamp.hour + timestamp.minute / 60.0
        diurnal_factor = 0.7 + 0.6 * ((hour - 14.0) / 12.0) ** 2
        gust = self.wind_gust_factor[bid] * random.uniform(0.0, 1.5)
        seasonal = 1.0 + 0.2 * random.gauss(0, 0.3)
        wind_speed = max(0.5, base * diurnal_factor * seasonal + gust * random.gauss(0, 1))

        direction = (timestamp.month * 30.0 + random.gauss(0, 40.0)) % 360.0
        attack_angle = random.gauss(0.0, 2.0) + wind_speed / 50.0 * random.choice([-1, 1]) * 3.0
        temperature = 15.0 + 10.0 * random.gauss(0, 0.8) + 5.0 * ((hour - 14.0) / 12.0)
        humidity = 65.0 + random.gauss(0, 15.0)

        return WindReading(
            sensor_id=f"W{bid[-3:]}01",
            speed=round(min(wind_speed, bridge_info["design_wind_speed"] * 1.5), 2),
            direction=round(direction % 360.0, 2),
            attack_angle=round(max(-15.0, min(15.0, attack_angle)), 2),
            temperature=round(temperature, 2),
            humidity=round(max(10.0, min(100.0, humidity)), 2),
        )

    def generate_cable_forces(self, bridge_info: Dict, wind: WindReading, temp: float) -> List[CableForceReading]:
        bid = bridge_info["bridge_id"]
        nominal = BASE_NOMINAL_FORCE.get(bid, 400000)
        forces = []
        wind_lift = wind.speed ** 2 * 0.08 * bridge_info["span"]
        temp_correction = 1.0 + (temp - 20.0) * 0.00012

        for i in range(bridge_info["cable_count"]):
            cid = f"C{i+1:02d}"
            offset = self.cable_force_offset[bid][cid]
            position_factor = 1.0 - 0.1 * abs(i - bridge_info["cable_count"] / 2) / (bridge_info["cable_count"] / 2)
            wind_component = wind_lift * position_factor / bridge_info["cable_count"] * random.gauss(1, 0.15)
            force = nominal * (1.0 + offset) * temp_correction + wind_component
            force += random.gauss(0, nominal * 0.005)
            forces.append(CableForceReading(
                cable_id=cid,
                force=round(force, 2),
                temperature=round(temp + random.gauss(0, 0.5), 2),
            ))
        return forces

    def generate_accelerations(self, bridge_info: Dict, wind: WindReading) -> List[AccelerationReading]:
        bid = bridge_info["bridge_id"]
        accs = []
        for idx, pos in enumerate(ACCELERATION_SENSOR_POSITIONS):
            mode_shape = abs((pos - 0.5) * 2) ** 2
            wind_induced = (wind.speed / 10.0) ** 2 * 0.05
            vibration_freq = 1.2 * (9.81 / bridge_info["span"]) ** 0.5
            phase = random.uniform(0, 6.283)
            az = wind_induced * mode_shape * random.gauss(1, 0.3) * (1.0 + 0.5 * random.gauss(0, 1))
            ax = az * 0.3 * random.gauss(0, 1)
            ay = az * 0.2 * random.gauss(0, 1)
            accs.append(AccelerationReading(
                sensor_id=f"A{bid[-3:]}{idx+1:02d}",
                position_x=round(pos * bridge_info["span"], 2),
                ax=round(ax, 4),
                ay=round(ay, 4),
                az=round(az, 4),
            ))
        return accs

    def generate_payload(self, bridge_info: Dict, now: Optional[datetime] = None) -> DTUPayload:
        if now is None:
            now = datetime.now(timezone.utc)
        bid = bridge_info["bridge_id"]
        wind = self.generate_wind(bridge_info, now)
        cable_forces = self.generate_cable_forces(bridge_info, wind, wind.temperature)
        accelerations = self.generate_accelerations(bridge_info, wind)
        return DTUPayload(
            device_id=f"DTU-{bid}",
            bridge_id=bid,
            timestamp=now.strftime("%Y-%m-%dT%H:%M:%S.") + f"{now.microsecond // 1000:03d}Z",
            cable_forces=cable_forces,
            accelerations=accelerations,
            wind=wind,
        )

    def send_payload(self, payload: DTUPayload) -> bool:
        try:
            conn = http.client.HTTPConnection(self.api_host, self.api_port, timeout=10)
            body = json.dumps(asdict(payload))
            headers = {"Content-Type": "application/json"}
            conn.request("POST", "/api/v1/dtu/receive", body, headers)
            resp = conn.getresponse()
            data = resp.read()
            conn.close()
            if resp.status == 200:
                result = json.loads(data)
                if result.get("success"):
                    return True
            print(f"[WARN] {payload.bridge_id} upload failed: HTTP {resp.status}")
            return False
        except Exception as e:
            print(f"[ERROR] {payload.bridge_id} connection failed: {e}")
            return False

    def bridge_worker(self, bridge_info: Dict):
        print(f"[START] Simulator for {bridge_info['name']} ({bridge_info['bridge_id']})")
        while not self.stop_event.is_set():
            now = datetime.now(timezone.utc)
            payload = self.generate_payload(bridge_info, now)
            success = self.send_payload(payload)
            status = "OK" if success else "FAIL"
            wind_info = f"wind={payload.wind.speed:.1f}m/s @{payload.wind.direction:.0f}°"
            max_force = max(cf.force for cf in payload.cable_forces) / 1000.0
            max_acc = max(abs(a.az) for a in payload.accelerations)
            print(f"[{now.strftime('%H:%M:%S')}] {bridge_info['bridge_id']} {status} | "
                  f"{wind_info} | max_force={max_force:.0f}kN | max_acc={max_acc:.3f}g")
            if self.stop_event.wait(self.interval):
                break
        print(f"[STOP] Simulator for {bridge_info['name']}")

    def start(self, bridges: Optional[List[str]] = None):
        target_bridges = BRIDGES if not bridges else [b for b in BRIDGES if b["bridge_id"] in bridges]
        print(f"\n{'='*60}")
        print("  4G DTU 桥梁传感器模拟器启动")
        print(f"  目标桥梁: {len(target_bridges)} 座")
        print(f"  上报间隔: {self.interval} 秒")
        print(f"  API 地址: http://{self.api_host}:{self.api_port}")
        print(f"{'='*60}\n")

        for bridge in target_bridges:
            t = threading.Thread(target=self.bridge_worker, args=(bridge,), daemon=True)
            t.start()
            self.threads.append(t)
            time.sleep(0.2)

        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print("\n\n[SHUTDOWN] 正在停止模拟器...")
            self.stop_event.set()
            for t in self.threads:
                t.join(timeout=5)
            print("[SHUTDOWN] 模拟器已停止")

    def single_shot(self, bridge_id: Optional[str] = None, print_only: bool = False):
        if bridge_id:
            bridges = [b for b in BRIDGES if b["bridge_id"] == bridge_id]
            if not bridges:
                print(f"Bridge {bridge_id} not found")
                return
        else:
            bridges = BRIDGES

        for bridge in bridges:
            payload = self.generate_payload(bridge)
            if print_only:
                print(f"\n=== {bridge['name']} ({bridge['bridge_id']}) ===")
                print(json.dumps(asdict(payload), indent=2, ensure_ascii=False))
            else:
                self.send_payload(payload)

def main():
    import argparse
    parser = argparse.ArgumentParser(description="古代悬索桥传感器模拟器")
    parser.add_argument("--host", default="localhost", help="API主机地址")
    parser.add_argument("--port", type=int, default=8080, help="API端口")
    parser.add_argument("--interval", type=int, default=60, help="上报间隔(秒)，默认60s便于演示")
    parser.add_argument("--bridges", nargs="*", help="指定模拟的桥梁ID列表 (如 BS001 BS002)")
    parser.add_argument("--once", action="store_true", help="只发送一次数据")
    parser.add_argument("--print-only", action="store_true", help="只打印数据，不上传")
    parser.add_argument("--bridge", type=str, help="指定单个桥梁ID (与--once配合)")
    args = parser.parse_args()

    sim = BridgeSensorSimulator(args.host, args.port, args.interval)

    if args.once:
        sim.single_shot(args.bridge, args.print_only)
    else:
        sim.start(args.bridges)

if __name__ == "__main__":
    main()
