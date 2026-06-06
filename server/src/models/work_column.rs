//! Work column configuration — per-user schema for the work_tasks table.
//!
//! See SPEC: `C:\Project\ergouPM\specs\work-task-table\spec.md`
//!
//! On first access for a given user, the backend seeds the 9 built-in columns
//! (see `builtin_seed`). Users can rename them, change types, edit options,
//! reorder them, and add/remove custom columns.
//!
//! - `builtin = true`  → cannot be deleted (the 9 system fixtures).
//! - `sys     = true`  → cannot have options edited (status/priority — options
//!   semantics are fixed by the app: status drives the board columns, priority
//!   drives the colored pills).

use serde::{Deserialize, Serialize};

/// One column in the work-task table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkColumn {
    pub key: String,
    pub name: String,
    /// One of: text / longtext / select / multi / number / percent / date / checkbox / status
    #[serde(rename = "type")]
    pub col_type: String,
    /// Option labels for select / multi types. Empty for other types.
    #[serde(default)]
    pub options: Vec<String>,
    /// Current width in px. None means "not set" (rare; clients fall back to a default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    /// Minimum drag width in px.
    #[serde(rename = "minWidth", skip_serializing_if = "Option::is_none")]
    pub min_width: Option<i32>,
    pub position: i32,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub sys: bool,
}

/// POST /api/work/columns — create a new custom column.
#[derive(Debug, Deserialize)]
pub struct CreateColumnRequest {
    pub name: String,
    #[serde(rename = "type", default = "default_col_type")]
    pub col_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(rename = "minWidth", default)]
    pub min_width: Option<i32>,
}

/// PUT /api/work/columns — batch save (after reorder, rename, type/options change).
///
/// Each item must include `key`; the rest are optional and only update fields
/// that are present. `position` controls ordering after save.
#[derive(Debug, Deserialize)]
pub struct BatchSaveColumnsRequest {
    pub columns: Vec<ColumnPatch>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnPatch {
    pub key: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub col_type: Option<String>,
    pub options: Option<Vec<String>>,
    pub width: Option<i32>,
    #[serde(rename = "minWidth")]
    pub min_width: Option<i32>,
    pub position: Option<i32>,
}

fn default_col_type() -> String {
    "text".to_string()
}

/// The 9 built-in columns seeded for each new user.
/// Mirrors `WT_COLS` in `frontend/work-board-preview.html`.
pub fn builtin_seed() -> Vec<WorkColumn> {
    fn level_opts() -> Vec<String> {
        vec!["院".into(), "所".into(), "组".into(), "个人".into()]
    }
    fn freq_opts() -> Vec<String> {
        vec!["一次性".into(), "每日".into(), "每周".into(), "每月".into()]
    }
    fn area_opts() -> Vec<String> {
        vec![
            "学术生态建设".into(),
            "业务竞争力构建".into(),
            "组织与人才".into(),
            "平台支撑运营".into(),
            "对外关系(总部/海外/周边)".into(),
            "洞察战略".into(),
        ]
    }
    let mut p = 0i32;
    let mut next = || {
        let v = p;
        p += 1;
        v
    };
    vec![
        WorkColumn {
            // T-095: 默认 320→260 让 9 列内置在 1366 屏宽一屏可见;min 不变,用户可拖回去
            key: "title".into(),
            name: "任务".into(),
            col_type: "text".into(),
            options: vec![],
            width: Some(260),
            min_width: Some(200),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            // T-095: 240→200(简介本来就靠展开按钮看全文)
            key: "desc".into(),
            name: "简介".into(),
            col_type: "longtext".into(),
            options: vec![],
            width: Some(200),
            min_width: Some(140),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "assignee".into(),
            name: "责任人".into(),
            col_type: "text".into(),
            options: vec![],
            width: Some(132),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "level".into(),
            name: "层级".into(),
            col_type: "select".into(),
            options: level_opts(),
            width: Some(88),
            min_width: Some(70),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "freq".into(),
            name: "频率".into(),
            col_type: "select".into(),
            options: freq_opts(),
            width: Some(96),
            min_width: Some(80),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "status".into(),
            name: "状态".into(),
            col_type: "status".into(),
            options: vec![], // sys: options are hardcoded on frontend (WT_STATUS)
            width: Some(104),
            min_width: Some(90),
            position: next(),
            builtin: true,
            sys: true,
        },
        WorkColumn {
            key: "priority".into(),
            name: "优先级".into(),
            col_type: "select".into(),
            options: vec![], // sys: options are hardcoded on frontend (WT_PRIO)
            width: Some(78),
            min_width: Some(60),
            position: next(),
            builtin: true,
            sys: true,
        },
        WorkColumn {
            key: "due".into(),
            name: "截止日".into(),
            col_type: "date".into(),
            options: vec![],
            width: Some(96),
            min_width: Some(80),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            // T-095: 130→110(进度条 + 数字占不到 130)
            key: "progress".into(),
            name: "进度".into(),
            col_type: "percent".into(),
            options: vec![],
            width: Some(110),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            // T-108: 内置「标签」列(multi 类型),为分布视图开箱即用。
            // 老用户通过 `ensure_builtin_present` 幂等补行(routes/work_columns.rs)。
            key: "tags".into(),
            name: "标签".into(),
            col_type: "multi".into(),
            options: vec![],
            width: Some(140),
            min_width: Some(80),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "area".into(),
            name: "领域".into(),
            col_type: "select".into(),
            options: area_opts(),
            width: Some(150),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        WorkColumn {
            key: "engage".into(),
            name: "我的角色".into(),
            col_type: "select".into(),
            options: vec![
                "decide".into(),
                "push".into(),
                "do".into(),
                "track".into(),
                "inform".into(),
            ],
            width: Some(112),
            min_width: Some(90),
            position: next(),
            builtin: true,
            sys: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_has_12_columns() {
        let s = builtin_seed();
        assert_eq!(s.len(), 12);
    }

    #[test]
    fn tags_column_is_multi_with_empty_options() {
        let s = builtin_seed();
        let tags = s.iter().find(|c| c.key == "tags").expect("tags not seeded");
        assert_eq!(tags.col_type, "multi");
        assert!(tags.options.is_empty());
        assert!(tags.builtin);
        assert!(!tags.sys);
        assert_eq!(tags.position, 9);
    }

    #[test]
    fn seed_positions_are_sequential() {
        let s = builtin_seed();
        for (i, c) in s.iter().enumerate() {
            assert_eq!(c.position, i as i32);
        }
    }

    #[test]
    fn status_priority_are_sys() {
        let s = builtin_seed();
        for c in &s {
            if c.key == "status" || c.key == "priority" || c.key == "engage" {
                assert!(c.sys, "{} should be sys", c.key);
            } else {
                assert!(!c.sys, "{} should not be sys", c.key);
            }
        }
    }

    #[test]
    fn area_and_engage_columns_are_seeded_for_owner_view() {
        let s = builtin_seed();
        let area = s.iter().find(|c| c.key == "area").expect("area not seeded");
        assert_eq!(area.col_type, "select");
        assert!(area.builtin);
        assert!(!area.sys);
        assert_eq!(area.position, 10);

        let engage = s
            .iter()
            .find(|c| c.key == "engage")
            .expect("engage not seeded");
        assert_eq!(engage.col_type, "select");
        assert_eq!(
            engage.options,
            vec!["decide", "push", "do", "track", "inform"]
        );
        assert!(engage.builtin);
        assert!(engage.sys);
        assert_eq!(engage.position, 11);
    }

    #[test]
    fn level_freq_have_options() {
        let s = builtin_seed();
        let level = s.iter().find(|c| c.key == "level").unwrap();
        assert_eq!(level.options.len(), 4);
        assert!(level.options.contains(&"院".to_string()));
        let freq = s.iter().find(|c| c.key == "freq").unwrap();
        assert_eq!(freq.options.len(), 4); // T-139:删「每季」后剩 一次性/每日/每周/每月
        assert!(!freq.options.contains(&"每季".to_string()));
    }

    #[test]
    fn all_seeds_are_builtin() {
        for c in builtin_seed() {
            assert!(c.builtin, "{} should be builtin", c.key);
        }
    }

    #[test]
    fn column_serde_renames_type_and_min_width() {
        let c = WorkColumn {
            key: "title".into(),
            name: "任务".into(),
            col_type: "text".into(),
            options: vec![],
            width: Some(320),
            min_width: Some(200),
            position: 0,
            builtin: true,
            sys: false,
        };
        let j = serde_json::to_string(&c).unwrap();
        assert!(j.contains("\"type\":\"text\""));
        assert!(j.contains("\"minWidth\":200"));
        assert!(!j.contains("col_type"));
        assert!(!j.contains("min_width"));
    }
}
