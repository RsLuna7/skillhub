use crate::config::AppConfig;
use crate::db::Database;
use anyhow::{Result, bail};
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::process::Command;

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub subject: String,
    pub status: String,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

impl Display for DoctorReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}: {}", self.subject, self.status)?;
        for check in &self.checks {
            writeln!(
                f,
                "- {:<28} {:<7} {}",
                check.name, check.status, check.detail
            )?;
        }
        Ok(())
    }
}

pub fn doctor_global(cfg: &AppConfig) -> Result<DoctorReport> {
    let mut checks = vec![
        path_check(
            "config",
            cfg.config_path.exists(),
            cfg.config_path.display().to_string(),
        ),
        path_check(
            "data_dir",
            cfg.data_dir_path()?.exists(),
            cfg.data_dir_path()?.display().to_string(),
        ),
        path_check(
            "default_install_dir",
            cfg.install_dir_path()?.exists(),
            cfg.install_dir_path()?.display().to_string(),
        ),
        path_check(
            "index_parent",
            cfg.index_path()
                .parent()
                .map(|p| p.exists())
                .unwrap_or(false),
            cfg.index_path().display().to_string(),
        ),
    ];
    for root in cfg.expanded_scan_roots() {
        checks.push(path_check(
            "scan_root",
            root.exists(),
            root.display().to_string(),
        ));
    }
    for cmd in ["git", "python", "node"] {
        checks.push(command_check(cmd));
    }
    if cfg!(windows) {
        checks.push(command_check("powershell"));
    }
    Ok(report("skillhub", checks))
}

pub fn doctor_skill(db: &Database, skill_id: &str) -> Result<DoctorReport> {
    let Some(skill) = db.get_skill(skill_id)? else {
        bail!("skill not found: {skill_id}");
    };
    let mut checks = vec![
        path_check(
            "install_path",
            skill.install_path.exists(),
            skill.install_path.display().to_string(),
        ),
        path_check(
            "entry_file",
            skill
                .entry_file
                .as_ref()
                .map(|f| skill.install_path.join(f).exists())
                .unwrap_or(false),
            skill.entry_file.unwrap_or_else(|| "(missing)".into()),
        ),
        path_check(
            "readme_file",
            skill
                .readme_file
                .as_ref()
                .map(|f| skill.install_path.join(f).exists())
                .unwrap_or(false),
            skill.readme_file.unwrap_or_else(|| "(missing)".into()),
        ),
        path_check(
            "runtime_conf",
            skill.install_path.join("runtime.conf").exists(),
            skill
                .install_path
                .join("runtime.conf")
                .display()
                .to_string(),
        ),
    ];
    for env in skill.required_env {
        checks.push(path_check(
            &format!("env:{env}"),
            std::env::var(&env).is_ok(),
            if std::env::var(&env).is_ok() {
                "set".into()
            } else {
                "missing".into()
            },
        ));
    }
    for command in db.get_commands(skill_id)? {
        checks.push(path_check(
            &format!("script:{}", command.runtime),
            skill.install_path.join(&command.source_file).exists(),
            command.source_file,
        ));
    }
    Ok(report(skill_id, checks))
}

fn report(subject: &str, checks: Vec<DoctorCheck>) -> DoctorReport {
    let status = if checks.iter().all(|c| c.status == "ok") {
        "ok"
    } else if checks.iter().any(|c| c.status == "error") {
        "error"
    } else {
        "warning"
    };
    DoctorReport {
        subject: subject.to_string(),
        status: status.to_string(),
        checks,
    }
}

fn path_check(name: &str, ok: bool, detail: String) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: if ok { "ok" } else { "warning" }.to_string(),
        detail,
    }
}

fn command_check(command: &str) -> DoctorCheck {
    let output = Command::new(command).arg("--version").output();
    match output {
        Ok(out) if out.status.success() => DoctorCheck {
            name: format!("command:{command}"),
            status: "ok".into(),
            detail: String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("available")
                .to_string(),
        },
        _ => DoctorCheck {
            name: format!("command:{command}"),
            status: "warning".into(),
            detail: "not found or not runnable".into(),
        },
    }
}
