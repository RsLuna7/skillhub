use crate::config::{AppConfig, init_config};
use crate::db::Database;
use crate::{audit, connect, demo, doctor, install, mcp, run as skill_run, scan, search, setup, trust};
use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "skillhub")]
#[command(about = "Lightweight local SkillHub for sharing agent skills across AI agents")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Setup(SetupArgs),
    Scan,
    List,
    Search {
        query: String,
        #[arg(long)]
        json: bool,
    },
    Show {
        skill_id: String,
        #[arg(long)]
        json: bool,
    },
    Doctor(DoctorArgs),
    Audit {
        skill_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Trust {
        #[command(subcommand)]
        command: TrustCommand,
    },
    Install {
        source: String,
    },
    Connect {
        agent: String,
        #[arg(long)]
        dry_run: bool,
    },
    Demo,
    Mcp,
    McpConfig,
    AgentInstructions {
        agent: Option<String>,
    },
    Run {
        skill_id: String,
        command_index: usize,
        #[arg(long)]
        dry_run: bool,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Args)]
struct DoctorArgs {
    skill_id: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SetupArgs {
    #[arg(long)]
    print_agent_instructions: bool,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Paths,
    AddPath { path: String },
}

#[derive(Debug, Subcommand)]
enum TrustCommand {
    List {
        #[arg(long)]
        json: bool,
    },
    Allow {
        skill_id: String,
    },
    Block {
        skill_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Reset {
        skill_id: String,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init => {
            let cfg = init_config()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            println!("Initialized SkillHub");
            println!("Config: {}", cfg.config_path.display());
            println!("Index: {}", cfg.index_path().display());
        }
        Command::Setup(args) => {
            setup::run_setup(args.print_agent_instructions)?;
        }
        Command::Scan => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            let report = scan::scan_all(&cfg, &db)?;
            println!(
                "Scanned {} roots, indexed {} skills",
                report.roots_scanned, report.skills_indexed
            );
        }
        Command::List => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            search::print_list(&db)?;
        }
        Command::Search { query, json } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            search::print_search(&db, &query, json)?;
        }
        Command::Show { skill_id, json } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            search::print_show(&db, &skill_id, json)?;
        }
        Command::Doctor(args) => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            match args.skill_id.as_deref() {
                Some("agents") => {
                    let home = home_dir();
                    let reports = doctor::doctor_agents(&home)?;
                    print_reports(&reports, args.json)?;
                }
                Some(agent) if doctor::AGENT_NAMES.contains(&agent) => {
                    let home = home_dir();
                    let report = doctor::doctor_agent(&home, agent)?;
                    print_report(&report, args.json)?;
                }
                Some(skill_id) => {
                    let report = doctor::doctor_skill(&db, skill_id)?;
                    print_report(&report, args.json)?;
                }
                None => {
                    let report = doctor::doctor_global(&cfg)?;
                    print_report(&report, args.json)?;
                }
            }
        }
        Command::Audit { skill_id, json } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            let reports = if let Some(skill_id) = skill_id {
                vec![audit::audit_skill(&db, &skill_id)?]
            } else {
                audit::audit_all(&db)?
            };
            print_reports(&reports, json)?;
        }
        Command::Trust { command } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            match command {
                TrustCommand::List { json } => trust::print_list(&db, json)?,
                TrustCommand::Allow { skill_id } => {
                    let record = trust::allow(&db, &skill_id)?;
                    println!("{}: {}", record.skill_id, record.status);
                }
                TrustCommand::Block { skill_id, reason } => {
                    let record = trust::block(&db, &skill_id, reason)?;
                    println!("{}: {}", record.skill_id, record.status);
                    println!("Hidden from MCP clients until unblocked.");
                }
                TrustCommand::Reset { skill_id } => {
                    trust::reset(&db, &skill_id)?;
                    println!("{skill_id}: untrusted");
                }
            }
        }
        Command::Install { source } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            let installed = install::install_github_skill(&cfg, &source)?;
            if let Some(skill_id) = installed.file_name().and_then(|name| name.to_str()) {
                db.record_install_source(skill_id, &install::normalize_github_url(&source)?)?;
            }
            let report = scan::scan_all(&cfg, &db)?;
            println!("Installed: {}", installed.display());
            println!("Indexed {} skills", report.skills_indexed);
        }
        Command::Connect { agent, dry_run } => {
            let home = home_dir();
            let command = setup::skillhub_command();
            let report =
                connect::connect(&home, &agent, &command.to_string_lossy(), dry_run)?;
            if !report.changed {
                println!(
                    "skillhub is already registered in {}",
                    report.config_path.display()
                );
            } else if dry_run {
                println!("Would update {}", report.config_path.display());
            } else {
                println!("Updated {}", report.config_path.display());
                if let Some(backup) = &report.backup_path {
                    println!("Backup written to {}", backup.display());
                }
                println!("Restart {} to pick up the change.", report.agent);
                println!();
                println!("{}", doctor::doctor_agent(&home, &agent)?);
            }
        }
        Command::Demo => {
            demo::run_demo()?;
        }
        Command::Mcp => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            mcp::serve_stdio(cfg, db)?;
        }
        Command::McpConfig => {
            println!(
                "{}",
                serde_json::json!({
                    "mcpServers": {
                        "skillhub": {
                            "command": "skillhub",
                            "args": ["mcp"]
                        }
                    }
                })
            );
        }
        Command::AgentInstructions { agent } => {
            println!("{}", setup::agent_instructions(agent.as_deref()));
        }
        Command::Run {
            skill_id,
            command_index,
            dry_run,
        } => {
            let cfg = AppConfig::load_or_init()?;
            let db = Database::open(&cfg)?;
            db.migrate()?;
            skill_run::print_run(&db, &skill_id, command_index, dry_run)?;
        }
        Command::Config { command } => {
            let mut cfg = AppConfig::load_or_init()?;
            match command {
                ConfigCommand::Paths => {
                    for root in &cfg.scan_roots {
                        println!("{root}");
                    }
                }
                ConfigCommand::AddPath { path } => {
                    if !cfg.scan_roots.iter().any(|p| p == &path) {
                        cfg.scan_roots.push(path);
                        cfg.save()?;
                    }
                    println!("Configured scan roots:");
                    for root in &cfg.scan_roots {
                        println!("{root}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn print_report<T: serde::Serialize + std::fmt::Display>(report: &T, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!("{report}");
    }
    Ok(())
}

fn print_reports<T: serde::Serialize + std::fmt::Display>(reports: &[T], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(reports)?);
        return Ok(());
    }
    for report in reports {
        println!("{report}");
    }
    Ok(())
}

fn home_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}
