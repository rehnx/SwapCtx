use crate::{paths, sensitive, storage, validate_text};
use anyhow::{bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use rusqlite::{params, Connection, TransactionBehavior};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Policy {
    Deny,
    Ask,
    Audit,
}
impl Policy {
    fn label(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
            Self::Audit => "audit",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Record a persistent boundary rule. Policies are evaluated only on request.
    Add {
        #[arg(long)]
        path: String,
        #[arg(long)]
        reason: String,
        #[arg(long, value_enum, default_value = "deny")]
        policy: Policy,
        #[arg(long)]
        allow_sensitive: bool,
    },
    List,
    /// Manual check: deny=2, ask=3, audit-only=0. No action is intercepted.
    Check {
        path: String,
    },
    Remove {
        id: i64,
    },
}

impl Action {
    pub fn writable(&self) -> bool {
        matches!(self, Self::Add { .. } | Self::Remove { .. })
    }
}

pub fn run(
    conn: &mut Connection,
    project: i64,
    cwd: &std::path::Path,
    root: &std::path::Path,
    action: Action,
) -> Result<u8> {
    match action {
        Action::Add {
            path,
            reason,
            policy,
            allow_sensitive,
        } => {
            paths::rule_matcher(&path)?;
            validate_text("Rule reason", &reason, 16 * 1024)?;
            sensitive::guard(&path, allow_sensitive)?;
            sensitive::guard(&reason, allow_sensitive)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM rules r LEFT JOIN rule_policies p ON p.rule_id=r.id
                WHERE r.project_id=?1 AND r.pattern=?2 AND r.reason=?3 AND coalesce(p.policy,'deny')=?4)", params![project,path,reason,policy.label()], |r| r.get(0))?;
            if exists {
                bail!("An identical rule already exists in this project");
            }
            tx.execute(
                "INSERT INTO rules(project_id, pattern, reason) VALUES (?1, ?2, ?3)",
                params![project, path, reason],
            )?;
            let rule = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO rule_policies(rule_id,policy) VALUES (?1,?2)",
                params![rule, policy.label()],
            )?;
            tx.commit()?;
            println!(
                "Recorded rule #{rule}: {} policy={}",
                sensitive::display(&path, false),
                policy.label()
            );
            println!("No agent interception or enforcement is active.");
        }
        Action::List | Action::Check { .. } => {
            let target = match &action {
                Action::Check { path } => Some(paths::target_paths(path, cwd, root)?),
                _ => None,
            };
            let sql = if storage::version(conn)? >= 2 {
                "SELECT r.id,r.pattern,r.reason,coalesce(p.policy,'deny') FROM rules r LEFT JOIN rule_policies p ON p.rule_id=r.id WHERE r.project_id=?1 ORDER BY r.id"
            } else {
                "SELECT id,pattern,reason,'deny' FROM rules WHERE project_id=?1 ORDER BY id"
            };
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([project])?;
            let mut seen = 0;
            let mut matched = false;
            let mut code = 0;
            while let Some(row) = rows.next()? {
                let rule: i64 = row.get(0)?;
                let pattern: String = row.get(1)?;
                let reason: String = row.get(2)?;
                let policy: String = row.get(3)?;
                if !matches!(policy.as_str(), "deny" | "ask" | "audit") {
                    bail!("Invalid stored policy for rule #{rule}; check failed");
                }
                seen += 1;
                if let Some(targets) = &target {
                    if paths::matches(&pattern, targets)
                        .with_context(|| format!("Invalid stored rule #{rule}; check failed"))?
                    {
                        let label = match policy.as_str() {
                            "deny" => "PROTECTED",
                            "ask" => "ASK",
                            _ => "AUDIT",
                        };
                        eprintln!(
                            "{label}: {} matches rule #{rule} '{}' policy={} - {}",
                            sensitive::display(&targets[0], false),
                            sensitive::display(&pattern, false),
                            policy,
                            sensitive::display(&reason, false)
                        );
                        matched = true;
                        if policy == "deny" {
                            code = 2;
                        } else if policy == "ask" && code != 2 {
                            code = 3;
                        }
                    }
                } else {
                    println!(
                        "#{rule} {} policy={policy} - {}",
                        sensitive::display(&pattern, false),
                        sensitive::display(&reason, false)
                    );
                }
            }
            if let Some(targets) = target {
                if !matched {
                    println!(
                        "No protected path rule matched: {}",
                        sensitive::display(&targets[0], false)
                    );
                }
                if code == 3 {
                    eprintln!("Approval required by recorded policy; SwapCtx has not requested or granted approval.");
                }
                return Ok(code);
            }
            if seen == 0 {
                println!("No protection rules yet.");
            }
        }
        Action::Remove { id } => {
            if conn.execute(
                "DELETE FROM rules WHERE id = ?1 AND project_id = ?2",
                params![id, project],
            )? == 0
            {
                bail!("Rule #{id} not found in this project");
            }
            println!("Removed rule #{id}");
        }
    }
    Ok(0)
}
