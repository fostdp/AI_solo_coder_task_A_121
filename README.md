# 古代悬索桥风致振动监测与气动优化系统

Ancient Suspension Bridge Wind-Induced Vibration Monitoring & Aerodynamic Optimization System

## 目录

- [系统架构](#系统架构)
- [快速开始](#快速开始)
- [模块说明](#模块说明)
- [传感器模拟器](#传感器模拟器)
- [API 文档](#api-文档)
- [监控与告警](#监控与告警)
- [配置说明](#配置说明)
- [部署指南](#部署指南)

---

## 系统架构

### 架构图

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                      客户端                                      │
│  ┌─────────────┐    ┌──────────────────┐    ┌─────────────┐    ┌─────────────┐ │
│  │  3D可视化   │    │   风振响应图表   │    │  优化面板   │    │  告警面板   │ │
│  │ (Three.js)  │    │   (Canvas 2D)    │    │ (GA参数)    │    │ (MQTT订阅)  │ │
│  └──────┬──────┘    └────────┬─────────┘    └──────┬──────┘    └──────┬──────┘ │
│         │                    │                       │                  │        │
│         └────────────────────┴───────────┬───────────┴──────────────────┘        │
│                                          │                                       │
│                                     cable_bridge_3d.js                           │
│                                     flutter_panel.js                              │
└──────────────────────────────────────────┼───────────────────────────────────────┘
                                           │ HTTP/REST API
┌──────────────────────────────────────────┼───────────────────────────────────────┐
│                                    Nginx (Gzip)                                  │
└──────────────────────────────────────────┼───────────────────────────────────────┘
                                           │
┌──────────────────────────────────────────▼───────────────────────────────────────┐
│                                Rust Backend (Actix-web)                           │
│                                                                                    │
│  ┌─────────────┐  mpsc  ┌──────────────────┐  mpsc  ┌─────────────┐              │
│  │ dtu_receiver│────────► flutter_analyzer │────────► alarm_mqtt  │───► MQTT     │
│  │ (数据采集)  │        │ (颤振分析)        │        │ (告警推送)   │              │
│  └──────┬──────┘        └────────┬─────────┘        └─────────────┘              │
│         │                        │                                                │
│         │                        └───────────────────┐                            │
│         ▼                                            ▼                            │
│  ┌─────────────┐        mpsc                 ┌─────────────┐                      │
│  │  InfluxDB   │◄─────────────────────────────┤  storage    │                      │
│  │ (时序存储)  │                             │  (worker)    │                      │
│  └─────────────┘                              └─────────────┘                      │
│                                                                                    │
│  ┌──────────────────────────┐      ┌──────────────────────────┐                    │
│  │  shape_optimizer         │      │  Prometheus Metrics      │                    │
│  │  (遗传算法优化)          │◄────►│  /metrics 端点          │                    │
│  │  + Kriging代理模型       │      │  (16种核心指标)          │                    │
│  └──────────────────────────┘      └──────────────────────────┘                    │
└────────────────────────────────────────────────────────────────────────────────────┘
           ▲
           │ HTTP POST /api/v1/dtu/receive
           │
┌──────────┴────────────────────────────────────────────────────────────────────────┐
│                                4G DTU 传感器网络                                 │
│                                                                                    │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐     ┌─────────┐                             │
│  │ BS001   │ │ BS002   │ │ BS003   │ ... │ BS010   │  10座古代铁索桥            │
│  │ 索力N点 │ │ 索力N点 │ │ 索力N点 │     │ 索力N点 │  每座桥:                    │
│  │ 加速度7 │ │ 加速度7 │ │ 加速度5 │     │ 加速度7 │    索力: 9~18测点          │
│  │ 风速2点 │ │ 风速3点 │ │ 风速1点 │     │ 风速2点 │    加速度: 5~7测点         │
│  └─────────┘ └─────────┘ └─────────┘     └─────────┘    风速: 1~3测点           │
└────────────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                          ┌──────────────────┐
                          │  增强版模拟器     │
                          │  - 10桥多测点     │
                          │  - 风速注入       │
                          │  - 紊流度注入     │
                          │  - 突风/风切变    │
                          └──────────────────┘
```

### 技术栈

| 层级 | 技术选型 |
|------|----------|
| **前端3D** | Three.js r128, GPU InstancedBufferGeometry, GLSL Shader |
| **前端图表** | Canvas 2D, 原生JavaScript (无框架依赖) |
| **后端** | Rust 1.80+, Actix-web 4, Tokio async runtime |
| **时序数据库** | InfluxDB 1.8 (带降采样连续查询) |
| **消息队列** | Eclipse Mosquitto 2.0 (MQTT 3.1.1) |
| **容器编排** | Docker Compose 3.8 |
| **监控** | Prometheus 2.53, Grafana 11.1 |
| **反向代理** | Nginx 1.27 (Gzip压缩, 缓存) |

### 核心算法

| 模块 | 算法 |
|------|------|
| **颤振分析** | Scanlan半经验颤振导数 + 准定常气动力 + Kalman滤波 + EMA平滑 + 置信区间估计 |
| **外形优化** | 遗传算法(锦标赛选择/均匀交叉/高斯变异) + Kriging代理模型(RBF核/LU求逆) + LHS采样 + EI加点准则 |
| **告警评估** | 三级阈值检测(注意/警告/危险) + 颤振裕度计算 |

---

## 快速开始

### 前置要求

- Docker 24.0+
- Docker Compose v2.20+
- 4GB+ 可用内存
- 10GB+ 可用磁盘空间

### 一键启动

```bash
# 克隆仓库
git clone <repository-url>
cd AI_solo_coder_task_A_121

# 启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f backend
```

### 访问地址

| 服务 | 地址 | 说明 |
|------|------|------|
| **前端界面** | http://localhost/ | 主监控界面 |
| **API文档** | http://localhost/api/v1/health | 健康检查 |
| **Prometheus** | http://localhost:9090/ | 指标监控 |
| **Grafana** | http://localhost:3000/ | 可视化面板 (admin/admin123) |
| **后端Metrics** | http://localhost:8080/metrics | Rust指标端点 |
| **InfluxDB** | http://localhost:8086/ | 时序数据库 |

### 验证安装

```bash
# 1. 检查健康状态
curl http://localhost/api/v1/health
# {"status":"healthy","timestamp":"..."}

# 2. 获取桥梁列表
curl http://localhost/api/v1/bridges

# 3. 测试颤振分析
curl "http://localhost/api/analyze?bridge_id=BS001&wind_speed=25&attack_angle=3"

# 4. 查看Prometheus指标
curl http://localhost:8080/metrics | head -50
```

### 停止服务

```bash
# 停止并保留数据
docker-compose down

# 停止并清除所有数据
docker-compose down -v
```

---

## 模块说明

### Rust 后端模块

#### 1. dtu_receiver - DTU数据接收器

**文件**: [backend/src/dtu_receiver.rs](backend/src/dtu_receiver.rs)

**职责**:
- 接收4G DTU上报的传感器数据
- 数据范围校验（索力、加速度、风速、紊流度）
- 写入存储队列
- 转发到颤振分析通道

**关键API**:
```rust
pub async fn process_payload(&self, payload: DTUPayload) -> Result<usize, String>
pub fn validate_payload(payload: &DTUPayload) -> Result<(), String>
```

#### 2. flutter_analyzer - 颤振分析器

**文件**: [backend/src/flutter_analyzer.rs](backend/src/flutter_analyzer.rs)

**职责**:
- 消费DTU数据消息
- 计算Scanlan颤振导数（带Kalman滤波+EMA平滑）
- 估算临界风速和颤振裕度
- 模型缓存（修复Kalman状态跨请求保持）
- 分发结果到告警和存储通道

**关键特性**:
- 支持多风传感器数据融合（取最大风速）
- 紊流强度自动传递到分析结果
- AerodynamicModel LRU缓存

#### 3. shape_optimizer - 外形优化器

**文件**: [backend/src/shape_optimizer.rs](backend/src/shape_optimizer.rs)

**职责**:
- 异步执行遗传算法优化
- Kriging代理模型加速适应度评估
- oneshot channel返回结果
- 120秒请求超时

**优化参数**:
- 风嘴角度 (0°~30°)
- 稳定板高度 (0~2m)
- 稳定板数量 (0~4)
- 导流板长度 (0~1m)
- 桥面开槽率 (0~0.3)

#### 4. alarm_mqtt - 告警服务

**文件**: [backend/src/alarm_mqtt.rs](backend/src/alarm_mqtt.rs)

**职责**:
- 三级阈值检测（注意/警告/危险）
- 告警去重和抑制
- MQTT消息发布
- 独立publisher worker线程

**告警级别**:
| 级别 | 条件 | MQTT Topic |
|------|------|------------|
| 注意 (INFO) | 颤振裕度 < 0.3 或 风速 > 0.7Ucr | `bridge/alerts/info` |
| 警告 (WARN) | 颤振裕度 < 0.2 或 风速 > 0.85Ucr | `bridge/alerts/warn` |
| 危险 (CRITICAL) | 颤振裕度 < 0.1 或 风速 > 0.95Ucr | `bridge/alerts/critical` |

---

## 传感器模拟器

### 特性

- ✅ 支持10座桥梁（BS001-BS010）
- ✅ 每座桥多测点：索力(9~18点)、加速度(5~7点)、风速(1~3点)
- ✅ 风速/风向/紊流度 CLI 参数注入
- ✅ 运行时交互模式：突风、风切变事件注入
- ✅ Docker友好：环境变量配置
- ✅ 自动统计：成功率、每桥发送统计

### 基本用法

```bash
# 本地运行（需要Python 3.10+）
cd scripts
pip install -r requirements.txt

# 启动所有10座桥，60秒间隔
python sensor_simulator.py --interval 60

# 只模拟特定桥梁
python sensor_simulator.py --bridges BS001 BS002 BS007 --interval 30

# 单次发送并打印数据
python sensor_simulator.py --once --bridge BS001 --print-only
```

### 高级用法 - 数据注入

```bash
# 注入固定风速 25m/s, 风向 90°, 紊流度 0.15
python sensor_simulator.py \
  --wind-speed 25.0 \
  --wind-direction 90.0 \
  --turbulence 0.15 \
  --target-bridge BS001 \
  --interval 10

# 为所有桥梁注入高风速 (测试告警)
python sensor_simulator.py \
  --wind-speed 40.0 \
  --turbulence 0.2 \
  --interval 10
```

### 交互模式

```bash
# 启动交互模式
python sensor_simulator.py --interactive --interval 30

# 运行时命令:
#  > gust BS001,BS007 2.5 120       # 给BS001和BS007注入2.5x突风，持续120秒
#  > shear BS001 1.8 60             # 给BS001注入1.8x风切变，持续60秒
#  > stats                           # 显示发送统计
#  > quit                            # 退出
```

### Docker 中运行

```bash
# 查看模拟器容器日志
docker-compose logs -f simulator

# 进入交互模式
docker attach bridge-simulator

# 或执行一次性命令
docker-compose run --rm simulator --once --bridge BS001 --print-only
```

### 测点分布

| 桥梁ID | 名称 | 索力测点 | 加速度测点 | 风速测点 | 跨径(m) |
|--------|------|----------|------------|----------|---------|
| BS001 | 泸定桥 | 13 | 7 | 2 | 100.0 |
| BS002 | 霁虹桥 | 18 | 7 | 3 | 106.0 |
| BS003 | 云龙桥 | 12 | 5 | 1 | 88.0 |
| BS004 | 重安江铁索桥 | 15 | 5 | 1 | 36.5 |
| BS005 | 盘江铁索桥 | 14 | 7 | 2 | 71.0 |
| BS006 | 程阳桥 | 10 | 5 | 1 | 58.0 |
| BS007 | 金龙桥 | 16 | 7 | 3 | 108.0 |
| BS008 | 豆沙关铁索桥 | 11 | 5 | 2 | 49.0 |
| BS009 | 普安桥 | 9 | 5 | 1 | 42.0 |
| BS010 | 安顺场铁索桥 | 12 | 7 | 2 | 62.0 |

---

## API 文档

### 基础路径

所有API路径前缀: `/api` 或 `/api/v1`

### 健康检查

```http
GET /api/v1/health
```

**响应**:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 桥梁管理

#### 获取桥梁列表

```http
GET /api/v1/bridges
```

**响应**:
```json
[
  {
    "bridge_id": "BS001",
    "name": "泸定桥",
    "span": 100.0,
    "design_wind_speed": 35.0,
    "cable_count": 13
  }
]
```

#### 获取单桥详情

```http
GET /api/v1/bridges/{id}
```

### 颤振分析

#### 评估气动性能

```http
GET /api/analyze?bridge_id={bridge_id}&wind_speed={speed}&attack_angle={angle}
GET /api/v1/aerodynamics/evaluate?bridge_id={bridge_id}&wind_speed={speed}&attack_angle={angle}
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| bridge_id | string | 是 | 桥梁ID (BS001-BS010) |
| wind_speed | float | 是 | 风速 m/s (0-150) |
| attack_angle | float | 否 | 攻角 -15°~15°，默认0° |

**响应**:
```json
{
  "bridge_id": "BS001",
  "wind_speed": 25.0,
  "attack_angle": 3.0,
  "aerodynamic_damping": 0.0023,
  "vibration_amplitude": 0.085,
  "flutter_critical_speed": 42.5,
  "flutter_margin": 0.412,
  "is_safe": true,
  "turbulence_intensity": 0.12,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 外形优化

#### 运行优化

```http
POST /api/optimize
POST /api/v1/optimization/run
Content-Type: application/json

{
  "bridge_id": "BS001",
  "population_size": 50,
  "generations": 30,
  "mutation_rate": 0.1,
  "crossover_rate": 0.8,
  "wind_speed_range": [10, 50],
  "attack_angle_range": [-5, 5]
}
```

**响应** (120秒超时):
```json
{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "bridge_id": "BS001",
  "best_shape": {
    "wind_nose_angle": 22.5,
    "stabilizer_plate_height": 1.2,
    "stabilizer_plate_count": 2,
    "fairing_length": 0.5,
    "porosity": 0.15
  },
  "flutter_critical_speed": 52.3,
  "improvement_percent": 23.1,
  "computation_time_ms": 45230
}
```

### DTU 数据上报

```http
POST /api/v1/dtu/receive
Content-Type: application/json

{
  "device_id": "DTU-BS001",
  "bridge_id": "BS001",
  "timestamp": "2024-01-15T10:30:00.000Z",
  "cable_forces": [
    {"cable_id": "C01", "force": 523400.5, "temperature": 12.3}
  ],
  "accelerations": [
    {"sensor_id": "A00101", "position_x": 10.0, "ax": 0.001, "ay": 0.002, "az": 0.015}
  ],
  "winds": [
    {"sensor_id": "W00101", "speed": 12.5, "direction": 90.0, "attack_angle": 2.1,
     "temperature": 12.3, "humidity": 65.0, "turbulence_intensity": 0.12}
  ],
  "event_type": "normal"
}
```

### 其他端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/aerodynamics/recent/{id}` | 获取最近的分析结果 |
| GET | `/api/v1/aerodynamics/flutter-curve/{id}` | 获取颤振临界风速曲线 |
| GET | `/api/v1/aerodynamics/vibration-response` | 获取风振响应时程 |
| GET | `/api/v1/aerodynamics/deck-deformation` | 获取桥面变形云图数据 |
| POST | `/api/v1/aerodynamics/evaluate-with-shape` | 评估特定外形的气动性能 |
| GET | `/metrics` | Prometheus指标端点 |

---

## 监控与告警

### Prometheus 指标

Rust后端暴露16种核心指标，访问 `http://localhost:8080/metrics`:

| 指标 | 类型 | 标签 | 说明 |
|------|------|------|------|
| `bridge_dtu_payloads_received_total` | Counter | bridge_id | DTU数据包接收数 |
| `bridge_dtu_payloads_valid_total` | Counter | bridge_id | 有效数据包数 |
| `bridge_dtu_payloads_invalid_total` | Counter | bridge_id, reason | 无效数据包数 |
| `bridge_aero_analyses_total` | Counter | bridge_id | 颤振分析总数 |
| `bridge_aero_analysis_duration_ms` | Histogram | bridge_id | 分析耗时分布 |
| `bridge_optimization_requests_total` | Counter | bridge_id, status | 优化请求数 |
| `bridge_optimization_duration_ms` | Histogram | bridge_id | 优化耗时分布 |
| `bridge_alerts_triggered_total` | Counter | bridge_id, severity | 告警触发数 |
| `bridge_mqtt_messages_published_total` | Counter | topic | MQTT消息发布数 |
| `bridge_influxdb_writes_total` | Counter | measurement | InfluxDB写入数 |
| `bridge_influxdb_write_errors_total` | Counter | measurement | InfluxDB写入错误数 |
| `bridge_active_connections` | Gauge | - | 活跃HTTP连接数 |
| `bridge_active_aero_models` | Gauge | - | 缓存的气动模型数 |
| `bridge_pending_optimizations` | Gauge | - | 待处理优化请求数 |

### Grafana 面板

1. 访问 http://localhost:3000 (admin/admin123)
2. 添加Prometheus数据源: `http://prometheus:9090`
3. 导入面板JSON或创建自定义面板

**推荐监控项**:
- 每桥DTU数据接收速率
- 颤振分析P95延迟
- 告警趋势（按级别/桥梁）
- InfluxDB写入成功率
- 活跃气动模型数

### MQTT 告警订阅

```bash
# 订阅所有告警
mosquitto_sub -h localhost -t 'bridge/alerts/#' -v

# 只订阅危险级别
mosquitto_sub -h localhost -t 'bridge/alerts/critical' -v
```

**告警消息格式**:
```json
{
  "bridge_id": "BS001",
  "severity": "critical",
  "message": "风速接近临界值",
  "wind_speed": 40.5,
  "flutter_critical_speed": 42.5,
  "flutter_margin": 0.047,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## 配置说明

### 气动参数配置

**文件**: [config/aero_params.json](config/aero_params.json)

```json
{
  "air_density": 1.225,
  "gravity_acceleration": 9.81,
  "flutter_derivatives_table": {
    "reduced_frequencies": [0.2, 0.5, 1.0, 2.0, 5.0, 10.0],
    "h_star": [0.0, ...],
    "a_star": [0.0, ...]
  },
  "kalman_filter": {
    "process_noise": 0.001,
    "measurement_noise": 0.01
  },
  "ema_smoothing": {
    "alpha": 0.3,
    "min_samples": 5
  },
  "confidence_interval": {
    "confidence_level": 0.95,
    "min_data_points": 10
  },
  "safety_criteria": {
    "flutter_margin_warning": 0.2,
    "flutter_margin_critical": 0.1,
    "max_amplitude": 0.2
  }
}
```

### 遗传算法配置

**文件**: [config/ga_params.json](config/ga_params.json)

```json
{
  "genetic_algorithm": {
    "population_size": 50,
    "max_generations": 30,
    "crossover_rate": 0.8,
    "mutation_rate": 0.1,
    "tournament_size": 3
  },
  "surrogate_model": {
    "enabled": true,
    "lhs_samples": 30,
    "rbf_kernel": "gaussian",
    "ei_exploration": 0.01
  },
  "parameter_bounds": {
    "wind_nose_angle": [0, 30],
    "stabilizer_plate_height": [0, 2.0],
    "stabilizer_plate_count": [0, 4],
    "fairing_length": [0, 1.0],
    "porosity": [0, 0.3]
  },
  "channel_buffers": {
    "dtu": 200,
    "analyzer": 200,
    "storage": 400
  }
}
```

### InfluxDB 保留策略

| 策略名 | 保留时长 | 降采样 | 用途 |
|--------|----------|--------|------|
| raw_data | 30天 | - | 原始高频数据 |
| hourly_agg | 365天 | 每小时均值/最大/最小/标准差 | 中期趋势分析 |
| daily_agg | 5年 | 每天聚合 | 长期历史分析 |

**连续查询自动执行**:
- `cq_hourly_cable_force`: 索力小时聚合
- `cq_hourly_acceleration`: 加速度小时聚合（含振动强度）
- `cq_hourly_wind`: 风速小时聚合
- `cq_daily_vibration`: 日振动强度统计

---

## 部署指南

### 生产环境部署

#### 1. 环境变量配置

复制 `.env` 并修改生产环境参数:

```bash
cp .env .env.production
vi .env.production
```

**关键生产配置**:
```env
# 使用强密码
INFLUXDB_PASS=your_strong_password
GRAFANA_ADMIN_PASSWORD=your_strong_password

# 生产日志级别
RUST_LOG=warn,bridge_monitoring_backend=info

# 增加缓冲区
CHANNEL_BUFFER=500

# MQTT启用认证
MQTT_USERNAME=bridge_user
MQTT_PASSWORD=mqtt_password
```

#### 2. Nginx 生产配置

[scripts/nginx.conf](scripts/nginx.conf) 已配置:
- ✅ Gzip压缩级别6（13种MIME类型）
- ✅ 静态资源1年缓存 + immutable
- ✅ HTML禁用缓存
- ✅ API超时配置（连接60s/读写120s）
- ✅ 安全头 (X-Frame-Options, X-XSS-Protection, X-Content-Type-Options)

#### 3. HTTPS 配置

建议使用 Let's Encrypt 或反向代理层加SSL:

```nginx
# 在nginx.conf中添加
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate /etc/nginx/ssl/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # ... 其他配置
}
```

#### 4. 资源限制

在 docker-compose.yml 中添加资源限制:

```yaml
services:
  backend:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 256M
```

### 备份与恢复

#### InfluxDB 数据备份

```bash
# 备份
docker exec bridge-influxdb influxd backup -portable -database bridge_monitoring /backup/$(date +%Y%m%d)

# 恢复
docker exec bridge-influxdb influxd restore -portable /backup/20240115
```

#### 定时备份

```bash
# 添加crontab
0 2 * * * docker exec bridge-influxdb influxd backup -portable -database bridge_monitoring /backup/$(date +\%Y\%m\%d)
```

### 性能调优

#### 1. Rust 后端性能

- 已启用 LTO + codegen-units=1 + opt-level=3
- 静态musl编译，无运行时依赖
- Channel缓冲可配置，避免背压

#### 2. InfluxDB 性能

[scripts/influxdb.conf](scripts/influxdb.conf) 已优化:
- 缓存大小 1GB
- 预检查询间隔 10分钟
- 最大系列数 100万
- 连续查询每秒运行

#### 3. 前端性能

- GPU实例化渲染风粒子（移动端58fps）
- Three.js 按需渲染
- Nginx Gzip压缩 + 长缓存

### 常见问题排查

#### 问题1: 后端启动失败，连接InfluxDB超时

```bash
# 检查InfluxDB状态
docker-compose ps influxdb
docker-compose logs influxdb

# 手动初始化数据库
docker exec bridge-influxdb influx -username admin -password admin123 \
  -execute "CREATE DATABASE bridge_monitoring"
```

#### 问题2: MQTT告警不推送

```bash
# 检查MQTT Broker状态
docker-compose ps mqtt-broker
mosquitto_pub -h localhost -t test -m "hello"

# 检查MQTT是否启用
grep MQTT_ENABLED .env
```

#### 问题3: 优化请求超时

优化请求默认120秒超时。如果优化时间过长:
- 减小种群大小 (`population_size`: 50 → 30)
- 减小代数 (`generations`: 30 → 20)
- 启用代理模型 (`surrogate_model.enabled: true`)

#### 问题4: 前端看不到3D模型

- 检查浏览器控制台是否有Three.js加载错误
- 确认 `cable_bridge_3d.js` 和 `flutter_panel.js` 加载顺序正确
- 检查 `index.html` 中的CDN链接是否可达

---

## 目录结构

```
AI_solo_coder_task_A_121/
├── backend/                          # Rust后端
│   ├── src/
│   │   ├── main.rs                  # 服务入口 + Channel装配
│   │   ├── dtu_receiver.rs          # DTU数据采集校验
│   │   ├── flutter_analyzer.rs      # 颤振分析 + 模型缓存
│   │   ├── shape_optimizer.rs       # 遗传算法优化
│   │   ├── alarm_mqtt.rs            # 告警评估 + MQTT推送
│   │   ├── aerodynamic_model.rs     # Scanlan颤振导数模型
│   │   ├── genetic_optimizer.rs     # GA + Kriging代理模型
│   │   ├── influxdb_storage.rs      # InfluxDB封装
│   │   ├── mqtt_alerts.rs           # MQTT客户端
│   │   ├── models.rs                # 数据结构 + SystemMessage
│   │   ├── handlers.rs              # HTTP Handler
│   │   └── metrics.rs               # Prometheus指标
│   ├── Cargo.toml
│   └── Dockerfile                   # 多阶段构建
├── frontend/                         # 前端代码
│   ├── index.html                   # 主页面
│   ├── cable_bridge_3d.js           # Three.js 3D场景
│   ├── flutter_panel.js             # 图表 + 告警 + 优化面板
│   └── style.css
├── config/                           # 外置配置
│   ├── aero_params.json             # 气动参数
│   └── ga_params.json               # 遗传算法参数
├── scripts/                          # 脚本与配置
│   ├── sensor_simulator.py          # 增强版模拟器
│   ├── init_influxdb.iql            # InfluxDB初始化 + 降采样
│   ├── influxdb.conf                # InfluxDB配置
│   ├── mosquitto.conf               # MQTT Broker配置
│   ├── nginx.conf                   # Nginx配置 (Gzip)
│   ├── prometheus.yml               # Prometheus监控配置
│   ├── Dockerfile.simulator         # 模拟器Dockerfile
│   └── requirements.txt
├── docker-compose.yml               # 服务编排
├── .env                             # 环境变量
└── README.md                        # 本文档
```

---

## 许可证

Copyright (c) 2024 桥梁史研究团队

---

## 技术支持

如有问题，请检查:
1. `docker-compose logs` 各服务日志
2. `/metrics` 端点指标
3. InfluxDB 中数据是否正常写入
4. 浏览器控制台前端错误
