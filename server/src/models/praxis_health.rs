//! Praxis 健康板块 (身体健康子板块) 数据模型 (T-296).
//!
//! See SPEC `C:\Project\ergouPM\specs\praxis\spec.md` §12 + 设计底本
//! `C:\Project\ergouPM\docs\praxis-health-dimensions-v1.md`.
//!
//! 三张记录表：`praxis_health_dims`(维度目录·用户可自定义) /
//! `praxis_health_marks`(每日习惯打卡) / `praxis_health_metrics`(周期指标·体检)
//! + `praxis_health_scores`(AI 打分快照)。字段 SQL 下划线 / API JSON 驼峰。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

/// 维度目录行：位置 = 优先级(ring) × 类别(sector)，采集方式 = kind。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDim {
    pub id: i64,
    #[serde(rename = "dimKey")]
    pub dim_key: String,
    pub name: String,
    pub sector: String,
    pub ring: String,
    pub kind: String,
    #[serde(default)]
    pub unit: String,
    #[serde(rename = "targetFloor", default)]
    pub target_floor: String,
    #[serde(rename = "targetGoal", default)]
    pub target_goal: String,
    #[serde(default)]
    pub cadence: String,
    #[serde(default)]
    pub seeded: bool,
    #[serde(rename = "sortOrder", default)]
    pub sort_order: f64,
    #[serde(default)]
    pub archived: bool,
}

/// 每日习惯打卡（每维每日一条，upsert）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMark {
    pub id: i64,
    #[serde(rename = "dimId")]
    pub dim_id: i64,
    #[serde(rename = "markDate")]
    pub mark_date: String,
    #[serde(default)]
    pub value: Option<f64>,
    pub done: bool,
    #[serde(default)]
    pub note: String,
}

/// 周期能力指标 / 体征 / 体检录入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetric {
    pub id: i64,
    #[serde(rename = "dimId")]
    pub dim_id: i64,
    #[serde(rename = "measuredAt")]
    pub measured_at: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(rename = "textValue", default)]
    pub text_value: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub note: String,
}

// AI 打分快照（dim_id=0=板块总分）以 `praxis_health_scores` 表存储，board 装配时
// 内联成 JSON（score/trend/explain），无需独立模型 struct。

// ---- 请求体 ----

#[derive(Debug, Deserialize, Default)]
pub struct CreateDimRequest {
    pub name: String,
    #[serde(default)]
    pub sector: Option<String>,
    #[serde(default)]
    pub ring: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(rename = "targetFloor", default)]
    pub target_floor: Option<String>,
    #[serde(rename = "targetGoal", default)]
    pub target_goal: Option<String>,
    #[serde(default)]
    pub cadence: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateDimRequest {
    #[serde(flatten)]
    pub fields: Map<String, JsonValue>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MarkRequest {
    #[serde(rename = "dimId")]
    pub dim_id: i64,
    #[serde(rename = "markDate", default)]
    pub mark_date: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default = "default_true")]
    pub done: bool,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
pub struct MetricRequest {
    #[serde(rename = "dimId")]
    pub dim_id: i64,
    #[serde(rename = "measuredAt", default)]
    pub measured_at: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(rename = "textValue", default)]
    pub text_value: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// 系统种子维度（严格对齐靶盘原型 `praxis-health-target-preview.html` 的 17 维，
/// 使靶盘位置一致）。字段：key, 名称, 类别, 圈层, 采集, 单位, 基线(floor)。
/// 采集：habit=每日打卡 / metric=周期自测·体检 / signal=派生·自评。
pub const SEED_DIMS: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    // 动
    (
        "move",
        "运动",
        "move",
        "core",
        "habit",
        "分钟",
        "WHO ≥150min 中强度/周 + 2×力量",
    ),
    (
        "sit",
        "久坐",
        "move",
        "core",
        "habit",
        "次",
        "每 45–60min 起身",
    ),
    (
        "flex",
        "柔韧",
        "move",
        "mid",
        "metric",
        "cm",
        "坐位体前屈 ≥0(过脚尖)",
    ),
    (
        "cardio",
        "心肺",
        "move",
        "watch",
        "metric",
        "",
        "12min 跑距/配速·年龄常模中档",
    ),
    (
        "strength",
        "力量",
        "move",
        "watch",
        "metric",
        "次",
        "俯卧撑/引体·年龄常模",
    ),
    (
        "power",
        "爆发",
        "move",
        "watch",
        "metric",
        "cm",
        "纵跳·年龄常模",
    ),
    // 吃
    (
        "nutrition",
        "饮食",
        "eat",
        "core",
        "habit",
        "",
        "蔬果/蛋白/低加工/规律",
    ),
    (
        "vitd",
        "维生素D",
        "eat",
        "core",
        "habit",
        "",
        "随餐服用(按体检 25-OH-D 定量)",
    ),
    ("water", "饮水", "eat", "mid", "habit", "升", "每日 1.5–2L"),
    // 睡·恢复
    (
        "light",
        "光照",
        "rest",
        "core",
        "habit",
        "分钟",
        "每日户外 20–30min(晨光)",
    ),
    (
        "recovery",
        "认知恢复",
        "rest",
        "core",
        "habit",
        "分钟",
        "每日无刺激恢复窗口",
    ),
    (
        "sleep",
        "睡眠",
        "rest",
        "mid",
        "habit",
        "小时",
        "7–9h·就寝波动<1h",
    ),
    // 体征·信号
    (
        "hairloss",
        "掉发",
        "sign",
        "mid",
        "signal",
        "",
        "追踪趋势(不诊断)",
    ),
    (
        "digestion",
        "消化",
        "sign",
        "mid",
        "signal",
        "",
        "无反复不适",
    ),
    (
        "energy",
        "精力",
        "sign",
        "mid",
        "signal",
        "1–5",
        "无持续 ≤2 / 均 ≥4",
    ),
    (
        "exam",
        "体检",
        "sign",
        "watch",
        "metric",
        "",
        "25-OH-D/铁蛋白/甲状腺/血脂",
    ),
    (
        "rhr",
        "静息心率",
        "sign",
        "watch",
        "metric",
        "bpm",
        "RHR<70(未接入·手动录)",
    ),
];

pub const SECTORS: &[&str] = &["move", "eat", "rest", "sign"];
pub const RINGS: &[&str] = &["core", "mid", "watch"];
pub const KINDS: &[&str] = &["habit", "metric", "signal"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_uses_camel_case() {
        let d = HealthDim {
            id: 1,
            dim_key: "light".into(),
            name: "光照".into(),
            sector: "rest".into(),
            ring: "core".into(),
            kind: "habit".into(),
            unit: "分钟".into(),
            target_floor: "20–30min".into(),
            target_goal: String::new(),
            cadence: "daily".into(),
            seeded: true,
            sort_order: 0.0,
            archived: false,
        };
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("dimKey"));
        assert!(j.contains("targetFloor"));
        assert!(!j.contains("dim_key"));
    }

    #[test]
    fn seed_catalog_is_well_formed() {
        assert_eq!(SEED_DIMS.len(), 17);
        for (_k, _n, sec, ring, kind, _u, _f) in SEED_DIMS {
            assert!(SECTORS.contains(sec), "bad sector {sec}");
            assert!(RINGS.contains(ring), "bad ring {ring}");
            assert!(KINDS.contains(kind), "bad kind {kind}");
        }
    }
}
