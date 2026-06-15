#!/usr/bin/env python3
"""
古代悬索桥风致振动监测系统 - 增强版传感器模拟器

Features:
- 支持10座桥梁（BS001-BS010）
- 每座桥多个测点：索力(N根)、加速度(7点)、风传感器(1~3点)
- 支持注入不同风速和紊流度（CLI参数）
- 支持模拟突风、风切变等极端天气事件
- Docker友好：支持环境变量配置
"""

import json
import os
import random
import sys
import time
import threading
import http.client
import urllib.parse
from datetime import datetime, timezone
from dataclasses import dataclass, asdict, field
from typing import List, Dict, Optional, Callable
from queue import Queue
import argparse

BRIDGES = [
    {"bridge_id": "BS001", "name": "泸定桥", "cable_count": 13, "span": 100.0, "design_wind_speed": 35.0, "acc_sensors": 7, "wind_sensors": 2},
    {"bridge_id": "BS002", "name": "霁虹桥", "cable_count": 18, "span": 106.0, "design_wind_speed": 32.0, "acc_sensors": 7, "wind_sensors": 3},
    {"bridge_id": "BS003", "name": "云龙桥", "cable_count": 12, "span": 88.0, "design_wind_speed": 30.0, "acc_sensors": 5, "wind_sensors": 1},
    {"bridge_id": "BS004", "name": "重安江铁索桥", "cable_count": 15, "span": 36.5, "design_wind_speed": 28.0, "acc_sensors": 5, "wind_sensors": 1},
    {"bridge_id": "BS005", "name": "盘江铁索桥", "cable_count": 14, "span": 71.0, "design_wind_speed": 38.0, "acc_sensors": 7, "wind_sensors": 2},
    {"bridge_id": "BS006", "name": "程阳桥", "cable_count": 10, "span": 58.0, "design_wind_speed": 25.0, "acc_sensors": 5, "wind_sensors": 1},
    {"bridge_id": "BS007", "name": "金龙桥", "cable_count": 16, "span": 108.0, "design_wind_speed": 40.0, "acc_sensors": 7, "wind_sensors": 3},
    {"bridge_id": "BS008", "name": "豆沙关铁索桥", "cable_count": 11, "span": 49.0, "design_wind_speed": 33.0, "acc_sensors": 5, "wind_sensors": 2},
    {"bridge_id": "BS009", "name": "普安桥", "cable_count": 9, "span": 42.0, "design_wind_speed": 29.0, "acc_sensors": 5, "wind_sensors": 1},
    {"bridge_id": "BS010", "name": "安顺场铁索桥", "cable_count": 12, "span": 62.0, "design_wind_speed": 31.0, "acc_sensors": 7, "wind_sensors": 2},
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
    turbulence_intensity: float = 0.1

@dataclass
class DTUPayload:
    device_id: str
    bridge_id: str
    timestamp: str
    cable_forces: List[CableForceReading]
    accelerations: List[AccelerationReading]
    winds: List[WindReading]
    event_type: str = "normal"

@dataclass
class WindProfile:
    base_speed: float
    speed_variance: float
    base_direction: float
    turbulence_intensity: float
    gust_factor: float
    diurnal_variation: bool = True
    seasonal_variation: bool = True

@dataclass
class ExtremeEvent:
    event_type: str
    start_time: float
    duration: float
    intensity: float
    affected_bridges: List[str]
    active: bool = False

    def is_active(self, current_time: float, bridge_id: str) -> bool:
        return (self.active and
                self.start_time <= current_time < self.start_time + self.duration and
                (not self.affected_bridges or bridge_id in self.affected_bridges))

class WindInjector:
    def __init__(self):
        self.overrides: Dict[str, WindProfile] = {}
        self.extreme_events: Queue = Queue()
        self.active_events: List[ExtremeEvent] = []

    def set_override(self, bridge_id: str, profile: WindProfile):
        self.overrides[bridge_id] = profile

    def clear_override(self, bridge_id: str):
        self.overrides.pop(bridge_id, None)

    def inject_gust(self, bridge_ids: List[str], intensity: float = 2.0, duration: float = 60.0):
        event = ExtremeEvent(
            event_type="gust",
            start_time=time.time(),
            duration=duration,
            intensity=intensity,
            affected_bridges=bridge_ids,
            active=True
        )
        self.active_events.append(event)
        print(f"[EVENT] 突风事件注入: 桥梁={bridge_ids}, 强度={intensity}x, 持续={duration}s")

    def inject_shear(self, bridge_ids: List[str], intensity: float = 1.5, duration: float = 120.0):
        event = ExtremeEvent(
            event_type="wind_shear",
            start_time=time.time(),
            duration=duration,
            intensity=intensity,
            affected_bridges=bridge_ids,
            active=True
        )
        self.active_events.append(event)
        print(f"[EVENT] 风切变事件注入: 桥梁={bridge_ids}, 强度={intensity}x, 持续={duration}s")

    def get_active_events(self, bridge_id: str) -> List[ExtremeEvent]:
        now = time.time()
        self.active_events = [e for e in self.active_events if e.is_active(now, bridge_id)]
        return self.active_events

    def get_profile(self, bridge_id: str, base_profile: WindProfile) -> WindProfile:
        if bridge_id in self.overrides:
            return self.overrides[bridge_id]
        return base_profile

    def apply_events(self, wind_speed: float, wind_dir: float, bridge_id: str) -> tuple:
        events = self.get_active_events(bridge_id)
        for event in events:
            if event.event_type == "gust":
                wind_speed *= (1 + event.intensity * random.uniform(0.5, 1.0))
            elif event.event_type == "wind_shear":
                wind_speed *= event.intensity
                wind_dir += random.uniform(-30, 30) * event.intensity
        return wind_speed, wind_dir % 360.0

class BridgeSensorSimulator:
    def __init__(self, api_host: str = "localhost", api_port: int = 8080,
                 interval_seconds: int = 600, wind_injector: Optional[WindInjector] = None):
        self.api_host = api_host
        self.api_port = api_port
        self.interval = interval_seconds
        self.stop_event = threading.Event()
        self.threads = []
        self.wind_injector = wind_injector or WindInjector()
        self.wind_base = {b["bridge_id"]: random.uniform(3.0, 12.0) for b in BRIDGES}
        self.wind_gust_factor = {b["bridge_id"]: random.uniform(0.0, 1.0) for b in BRIDGES}
        self.cable_force_offset = {}
        for b in BRIDGES:
            self.cable_force_offset[b["bridge_id"]] = {
                f"C{i+1:02d}": random.uniform(-0.02, 0.02) for i in range(b["cable_count"])
            }
        self.stats = {
            "total_sent": 0,
            "success": 0,
            "failed": 0,
            "by_bridge": {b["bridge_id"]: {"sent": 0, "success": 0} for b in BRIDGES}
        }

    def generate_wind(self, bridge_info: Dict, timestamp: datetime, sensor_idx: int = 0) -> WindReading:
        bid = bridge_info["bridge_id"]
        base = self.wind_base[bid]
        hour = timestamp.hour + timestamp.minute / 60.0

        base_profile = WindProfile(
            base_speed=base,
            speed_variance=0.3,
            base_direction=(timestamp.month * 30.0) % 360.0,
            turbulence_intensity=random.uniform(0.08, 0.15),
            gust_factor=self.wind_gust_factor[bid]
        )

        profile = self.wind_injector.get_profile(bid, base_profile)

        diurnal_factor = 0.7 + 0.6 * ((hour - 14.0) / 12.0) ** 2 if profile.diurnal_variation else 1.0
        seasonal = 1.0 + 0.2 * random.gauss(0, 0.3) if profile.seasonal_variation else 1.0
        gust = profile.gust_factor * random.uniform(0.0, 1.5)

        wind_speed = max(0.5, profile.base_speed * diurnal_factor * seasonal + gust * random.gauss(0, 1))
        wind_speed += random.gauss(0, profile.base_speed * profile.speed_variance * 0.1)

        direction = profile.base_direction + random.gauss(0, 40.0)
        attack_angle = random.gauss(0.0, 2.0) + wind_speed / 50.0 * random.choice([-1, 1]) * 3.0
        temperature = 15.0 + 10.0 * random.gauss(0, 0.8) + 5.0 * ((hour - 14.0) / 12.0)
        humidity = 65.0 + random.gauss(0, 15.0)

        wind_speed, direction = self.wind_injector.apply_events(wind_speed, direction, bid)

        height_factor = 1.0 + sensor_idx * 0.1

        return WindReading(
            sensor_id=f"W{bid[-3:]}{sensor_idx+1:02d}",
            speed=round(min(wind_speed * height_factor, bridge_info["design_wind_speed"] * 1.8), 2),
            direction=round(direction % 360.0, 2),
            attack_angle=round(max(-15.0, min(15.0, attack_angle)), 2),
            temperature=round(temperature, 2),
            humidity=round(max(10.0, min(100.0, humidity)), 2),
            turbulence_intensity=round(profile.turbulence_intensity * (1 + random.gauss(0, 0.2)), 3)
        )

    def generate_cable_forces(self, bridge_info: Dict, winds: List[WindReading], temp: float) -> List[CableForceReading]:
        bid = bridge_info["bridge_id"]
        nominal = BASE_NOMINAL_FORCE.get(bid, 400000)
        forces = []
        avg_wind_speed = sum(w.speed for w in winds) / max(1, len(winds))
        wind_lift = avg_wind_speed ** 2 * 0.08 * bridge_info["span"]
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

    def generate_accelerations(self, bridge_info: Dict, winds: List[WindReading]) -> List[AccelerationReading]:
        bid = bridge_info["bridge_id"]
        accs = []
        avg_wind_speed = sum(w.speed for w in winds) / max(1, len(winds))
        num_sensors = min(bridge_info["acc_sensors"], len(ACCELERATION_SENSOR_POSITIONS))

        for idx in range(num_sensors):
            pos = ACCELERATION_SENSOR_POSITIONS[idx]
            mode_shape = abs((pos - 0.5) * 2) ** 2
            wind_induced = (avg_wind_speed / 10.0) ** 2 * 0.05
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

        winds = [self.generate_wind(bridge_info, now, i) for i in range(bridge_info["wind_sensors"])]
        avg_temp = sum(w.temperature for w in winds) / max(1, len(winds))

        max_wind = max(w.speed for w in winds)
        design = bridge_info["design_wind_speed"]
        event_type = "normal"
        if max_wind > design * 1.2:
            event_type = "extreme_wind"
        elif max_wind > design * 0.9:
            event_type = "high_wind"

        cable_forces = self.generate_cable_forces(bridge_info, winds, avg_temp)
        accelerations = self.generate_accelerations(bridge_info, winds)

        return DTUPayload(
            device_id=f"DTU-{bid}",
            bridge_id=bid,
            timestamp=now.strftime("%Y-%m-%dT%H:%M:%S.") + f"{now.microsecond // 1000:03d}Z",
            cable_forces=cable_forces,
            accelerations=accelerations,
            winds=winds,
            event_type=event_type
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

            self.stats["total_sent"] += 1
            self.stats["by_bridge"][payload.bridge_id]["sent"] += 1

            if resp.status == 200:
                result = json.loads(data)
                if result.get("success"):
                    self.stats["success"] += 1
                    self.stats["by_bridge"][payload.bridge_id]["success"] += 1
                    return True
            self.stats["failed"] += 1
            print(f"[WARN] {payload.bridge_id} upload failed: HTTP {resp.status}")
            return False
        except Exception as e:
            self.stats["failed"] += 1
            print(f"[ERROR] {payload.bridge_id} connection failed: {e}")
            return False

    def bridge_worker(self, bridge_info: Dict):
        print(f"[START] Simulator for {bridge_info['name']} ({bridge_info['bridge_id']}) "
              f"[索力:{bridge_info['cable_count']}点, 加速度:{bridge_info['acc_sensors']}点, 风速:{bridge_info['wind_sensors']}点]")
        while not self.stop_event.is_set():
            now = datetime.now(timezone.utc)
            payload = self.generate_payload(bridge_info, now)
            success = self.send_payload(payload)
            status = "OK" if success else "FAIL"
            max_wind = max(w.speed for w in payload.winds)
            avg_wind_dir = sum(w.direction for w in payload.winds) / max(1, len(payload.winds))
            max_force = max(cf.force for cf in payload.cable_forces) / 1000.0
            max_acc = max(abs(a.az) for a in payload.accelerations)
            print(f"[{now.strftime('%H:%M:%S')}] {bridge_info['bridge_id']} {status} | "
                  f"wind={max_wind:.1f}m/s @{avg_wind_dir:.0f}° | "
                  f"max_force={max_force:.0f}kN | max_acc={max_acc:.3f}g | "
                  f"event={payload.event_type}")
            if self.stop_event.wait(self.interval):
                break
        print(f"[STOP] Simulator for {bridge_info['name']}")

    def print_stats(self):
        print(f"\n{'='*60}")
        print("  模拟器统计")
        print(f"{'='*60}")
        print(f"  总发送: {self.stats['total_sent']} | 成功: {self.stats['success']} | 失败: {self.stats['failed']}")
        for bid, s in self.stats["by_bridge"].items():
            if s["sent"] > 0:
                rate = s["success"] / s["sent"] * 100
                bridge = next(b for b in BRIDGES if b["bridge_id"] == bid)
                print(f"  {bid} {bridge['name']:12s}: {s['sent']:4d} sent, {s['success']:4d} ok ({rate:5.1f}%)")
        print(f"{'='*60}\n")

    def start(self, bridges: Optional[List[str]] = None):
        target_bridges = BRIDGES if not bridges else [b for b in BRIDGES if b["bridge_id"] in bridges]
        print(f"\n{'='*60}")
        print("  4G DTU 桥梁传感器模拟器 (增强版)")
        print(f"  目标桥梁: {len(target_bridges)} 座")
        print(f"  上报间隔: {self.interval} 秒")
        print(f"  API 地址: http://{self.api_host}:{self.api_port}")
        print(f"{'='*60}\n")

        for bridge in target_bridges:
            t = threading.Thread(target=self.bridge_worker, args=(bridge,), daemon=True)
            t.start()
            self.threads.append(t)
            time.sleep(0.2)

        stats_thread = threading.Thread(target=self._stats_printer, daemon=True)
        stats_thread.start()

        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print("\n\n[SHUTDOWN] 正在停止模拟器...")
            self.stop_event.set()
            for t in self.threads:
                t.join(timeout=5)
            self.print_stats()
            print("[SHUTDOWN] 模拟器已停止")

    def _stats_printer(self):
        while not self.stop_event.is_set():
            time.sleep(60)
            self.print_stats()

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

    def interactive_mode(self):
        """交互式模式，允许运行时注入事件"""
        print("\n[交互模式] 输入命令:")
        print("  gust <bridge_ids> <intensity> <duration>  - 注入突风事件")
        print("  shear <bridge_ids> <intensity> <duration> - 注入风切变事件")
        print("  stats                                      - 显示统计")
        print("  quit                                       - 退出")

        def cmd_loop():
            while not self.stop_event.is_set():
                try:
                    cmd = input("> ").strip().split()
                    if not cmd:
                        continue
                    if cmd[0] == "quit":
                        self.stop_event.set()
                        break
                    elif cmd[0] == "stats":
                        self.print_stats()
                    elif cmd[0] in ["gust", "shear"] and len(cmd) >= 3:
                        bridge_ids = cmd[1].split(",")
                        intensity = float(cmd[2])
                        duration = float(cmd[3]) if len(cmd) > 3 else 60.0
                        if cmd[0] == "gust":
                            self.wind_injector.inject_gust(bridge_ids, intensity, duration)
                        else:
                            self.wind_injector.inject_shear(bridge_ids, intensity, duration)
                    else:
                        print("未知命令")
                except (EOFError, KeyboardInterrupt):
                    self.stop_event.set()
                    break
                except Exception as e:
                    print(f"命令错误: {e}")

        t = threading.Thread(target=cmd_loop, daemon=True)
        t.start()

def main():
    parser = argparse.ArgumentParser(description="古代悬索桥传感器模拟器 (增强版)")
    parser.add_argument("--host", default=os.environ.get("API_HOST", "localhost"), help="API主机地址")
    parser.add_argument("--port", type=int, default=int(os.environ.get("API_PORT", "8080")), help="API端口")
    parser.add_argument("--interval", type=int, default=int(os.environ.get("SIM_INTERVAL", "60")), help="上报间隔(秒)")
    parser.add_argument("--bridges", nargs="*", help="指定模拟的桥梁ID列表 (如 BS001 BS002)")
    parser.add_argument("--once", action="store_true", help="只发送一次数据")
    parser.add_argument("--print-only", action="store_true", help="只打印数据，不上传")
    parser.add_argument("--bridge", type=str, help="指定单个桥梁ID (与--once配合)")
    parser.add_argument("--interactive", action="store_true", help="启动交互模式")

    parser.add_argument("--wind-speed", type=float, help="注入固定风速 (m/s)")
    parser.add_argument("--wind-direction", type=float, help="注入固定风向 (度)")
    parser.add_argument("--turbulence", type=float, help="注入紊流强度 (0.05-0.3)")
    parser.add_argument("--target-bridge", type=str, help="注入目标桥梁ID (默认所有)")

    args = parser.parse_args()

    wind_injector = WindInjector()

    if args.wind_speed or args.wind_direction or args.turbulence:
        from dataclasses import replace
        profile = WindProfile(
            base_speed=args.wind_speed or 8.0,
            speed_variance=0.1,
            base_direction=args.wind_direction or 0.0,
            turbulence_intensity=args.turbulence or 0.1,
            gust_factor=0.0,
            diurnal_variation=False,
            seasonal_variation=False
        )
        targets = [args.target_bridge] if args.target_bridge else [b["bridge_id"] for b in BRIDGES]
        for bid in targets:
            wind_injector.set_override(bid, profile)
            print(f"[INJECT] {bid}: 风速={profile.base_speed}m/s, 风向={profile.base_direction}°, 紊流={profile.turbulence_intensity}")

    sim = BridgeSensorSimulator(args.host, args.port, args.interval, wind_injector)

    if args.once:
        sim.single_shot(args.bridge, args.print_only)
    elif args.interactive:
        bridges = args.bridges if args.bridges else None
        threading.Thread(target=sim.start, args=(bridges,), daemon=True).start()
        sim.interactive_mode()
    else:
        sim.start(args.bridges)

if __name__ == "__main__":
    main()
