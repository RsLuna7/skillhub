use crate::config::AppConfig;
use crate::db::Database;
use crate::doctor;
use crate::security::safe_skill_file_path;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub fn serve_stdio(cfg: AppConfig, db: Database) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                write_response(
                    &mut stdout,
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":err.to_string()}}),
                )?;
                continue;
            }
        };
        if req
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .starts_with("notifications/")
        {
            continue;
        }
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let result = handle_request(&cfg, &db, &req);
        match result {
            Ok(value) => {
                write_response(&mut stdout, json!({"jsonrpc":"2.0","id":id,"result":value}))?
            }
            Err(err) => write_response(
                &mut stdout,
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":err.to_string()}}),
            )?,
        }
    }
    Ok(())
}

fn handle_request(cfg: &AppConfig, db: &Database, req: &Value) -> Result<Value> {
    match req.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "skillhub", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let params = req.get("params").unwrap_or(&Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let payload = call_tool(cfg, db, name, &args)?;
            Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&payload)? }],
                "isError": false
            }))
        }
        "" => bail!("missing method"),
        method => bail!("unsupported method: {method}"),
    }
}

fn tools() -> Value {
    json!([
        tool(
            "skillhub.search_skills",
            "Search indexed agent skills",
            json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]})
        ),
        tool(
            "skillhub.list_skills",
            "List indexed agent skills",
            json!({"type":"object","properties":{}})
        ),
        tool(
            "skillhub.get_skill",
            "Get skill summary and metadata",
            json!({"type":"object","properties":{"skill_id":{"type":"string"}},"required":["skill_id"]})
        ),
        tool(
            "skillhub.get_skill_file",
            "Read a safe file from a skill directory",
            json!({"type":"object","properties":{"skill_id":{"type":"string"},"file":{"type":"string"}},"required":["skill_id","file"]})
        ),
        tool(
            "skillhub.get_skill_commands",
            "Return recommended commands without executing them",
            json!({"type":"object","properties":{"skill_id":{"type":"string"}},"required":["skill_id"]})
        ),
        tool(
            "skillhub.doctor_skill",
            "Run structured diagnostics for one skill",
            json!({"type":"object","properties":{"skill_id":{"type":"string"}},"required":["skill_id"]})
        )
    ])
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn call_tool(cfg: &AppConfig, db: &Database, name: &str, args: &Value) -> Result<Value> {
    match name {
        "skillhub.search_skills" => {
            let query = required_str(args, "query")?;
            Ok(json!({ "results": db.search_skills(query)? }))
        }
        "skillhub.list_skills" => Ok(json!({ "skills": db.list_skills()? })),
        "skillhub.get_skill" => {
            let skill_id = required_str(args, "skill_id")?;
            Ok(json!({ "skill": db.get_skill(skill_id)? }))
        }
        "skillhub.get_skill_file" => {
            let skill_id = required_str(args, "skill_id")?;
            let file = required_str(args, "file")?;
            let Some(skill) = db.get_skill(skill_id)? else {
                bail!("skill not found: {skill_id}");
            };
            let path = safe_skill_file_path(&skill.install_path, file)?;
            let mut content = std::fs::read_to_string(path)?;
            let truncated = content.chars().count() > cfg.mcp.max_file_chars;
            if truncated {
                content = content
                    .chars()
                    .take(cfg.mcp.max_file_chars)
                    .collect::<String>();
            }
            Ok(
                json!({ "skill_id": skill_id, "file": file, "content": content, "truncated": truncated }),
            )
        }
        "skillhub.get_skill_commands" => {
            let skill_id = required_str(args, "skill_id")?;
            Ok(json!({ "commands": db.get_commands(skill_id)? }))
        }
        "skillhub.doctor_skill" => {
            let skill_id = required_str(args, "skill_id")?;
            Ok(json!(doctor::doctor_skill(db, skill_id)?))
        }
        _ => bail!("unknown tool: {name}"),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string argument: {key}"))
}

fn write_response(stdout: &mut io::Stdout, response: Value) -> Result<()> {
    writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
    stdout.flush()?;
    Ok(())
}
