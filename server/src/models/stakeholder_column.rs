//! Stakeholder column configuration — per-user schema for the stakeholder table.
//!
//! See SPEC: `C:\Project\ergouPM\specs\stakeholder\spec.md`

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeholderColumn {
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub col_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(rename = "minWidth", skip_serializing_if = "Option::is_none")]
    pub min_width: Option<i32>,
    pub position: i32,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub sys: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateStakeholderColumnRequest {
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

#[derive(Debug, Deserialize)]
pub struct BatchSaveStakeholderColumnsRequest {
    pub columns: Vec<StakeholderColumnPatch>,
}

#[derive(Debug, Deserialize)]
pub struct StakeholderColumnPatch {
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

pub fn builtin_seed() -> Vec<StakeholderColumn> {
    fn method_opts() -> Vec<String> {
        vec![
            "定期汇报".into(),
            "定期吃饭".into(),
            "定期拜访".into(),
            "电话沟通".into(),
        ]
    }
    fn cadence_opts() -> Vec<String> {
        vec!["每周".into(), "每月".into(), "每季".into(), "不定期".into()]
    }
    let mut p = 0i32;
    let mut next = || {
        let v = p;
        p += 1;
        v
    };
    vec![
        StakeholderColumn {
            key: "name".into(),
            name: "姓名".into(),
            col_type: "text".into(),
            options: vec![],
            width: Some(140),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "team".into(),
            name: "部门/团队".into(),
            col_type: "select".into(),
            options: vec![],
            width: Some(140),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "region".into(),
            name: "地域".into(),
            col_type: "select".into(),
            options: vec![],
            width: Some(110),
            min_width: Some(80),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "title".into(),
            name: "职务".into(),
            col_type: "text".into(),
            options: vec![],
            width: Some(140),
            min_width: Some(100),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "duty".into(),
            name: "负责事项".into(),
            col_type: "multi".into(),
            options: vec![],
            width: Some(180),
            min_width: Some(120),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "liaison".into(),
            name: "接口事项".into(),
            col_type: "multi".into(),
            options: vec![],
            width: Some(180),
            min_width: Some(120),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "relation".into(),
            name: "关系".into(),
            col_type: "longtext".into(),
            options: vec![],
            width: Some(220),
            min_width: Some(140),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "method".into(),
            name: "管理方式".into(),
            col_type: "multi".into(),
            options: method_opts(),
            width: Some(160),
            min_width: Some(110),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "cadence".into(),
            name: "频率".into(),
            col_type: "select".into(),
            options: cadence_opts(),
            width: Some(100),
            min_width: Some(80),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "strategy".into(),
            name: "建议策略".into(),
            col_type: "longtext".into(),
            options: vec![],
            width: Some(220),
            min_width: Some(140),
            position: next(),
            builtin: true,
            sys: false,
        },
        StakeholderColumn {
            key: "notes".into(),
            name: "备注".into(),
            col_type: "longtext".into(),
            options: vec![],
            width: Some(200),
            min_width: Some(120),
            position: next(),
            builtin: true,
            sys: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stakeholder_seed_has_11_builtin_columns() {
        let s = builtin_seed();
        assert_eq!(s.len(), 11);
        for (i, c) in s.iter().enumerate() {
            assert_eq!(c.position, i as i32);
            assert!(c.builtin);
            assert!(!c.sys);
        }
    }

    #[test]
    fn method_and_cadence_options_are_seeded() {
        let s = builtin_seed();
        let method = s.iter().find(|c| c.key == "method").unwrap();
        assert_eq!(method.col_type, "multi");
        assert!(method.options.contains(&"定期汇报".to_string()));
        let cadence = s.iter().find(|c| c.key == "cadence").unwrap();
        assert_eq!(cadence.col_type, "select");
        assert!(cadence.options.contains(&"每季".to_string()));
    }
}
