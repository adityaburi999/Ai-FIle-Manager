use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection};

use crate::models::{ActionLog, ExclusionMode, ExclusionRule};

pub fn init(db_path: &Path) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("failed opening sqlite db at {}", db_path.display()))?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            content_preview TEXT NOT NULL,
            hash TEXT NOT NULL,
            last_indexed TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exclusions (
            path TEXT PRIMARY KEY,
            excluded INTEGER NOT NULL,
            mode TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS action_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            action TEXT NOT NULL,
            source TEXT NOT NULL,
            destination TEXT NOT NULL,
            reason TEXT NOT NULL,
            model_confidence REAL NOT NULL,
            rollback_group TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS move_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rollback_group TEXT NOT NULL,
            source TEXT NOT NULL,
            destination TEXT NOT NULL
        );
        "#,
    )?;

    Ok(())
}

pub fn upsert_file_record(
    db_path: &Path,
    path: &str,
    category: &str,
    sub_category: &str,
    tags_json: &str,
    content_preview: &str,
    hash: &str,
    last_indexed: &str,
) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        r#"
        INSERT INTO files(path, category, sub_category, tags_json, content_preview, hash, last_indexed)
        VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(path) DO UPDATE SET
            category=excluded.category,
            sub_category=excluded.sub_category,
            tags_json=excluded.tags_json,
            content_preview=excluded.content_preview,
            hash=excluded.hash,
            last_indexed=excluded.last_indexed
        "#,
        params![path, category, sub_category, tags_json, content_preview, hash, last_indexed],
    )?;
    Ok(())
}

pub fn set_exclusion_rule(db_path: &Path, rule: &ExclusionRule) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)?;
    let mode = match rule.mode {
        ExclusionMode::Ignore => "ignore",
        ExclusionMode::ReadOnly => "read_only",
        ExclusionMode::Manual => "manual",
    };

    conn.execute(
        r#"
        INSERT INTO exclusions(path, excluded, mode)
        VALUES(?1, ?2, ?3)
        ON CONFLICT(path) DO UPDATE SET excluded=excluded.excluded, mode=excluded.mode
        "#,
        params![rule.path, i64::from(rule.excluded), mode],
    )?;

    Ok(())
}

pub fn get_matching_exclusion_mode(db_path: &Path, target_path: &str) -> anyhow::Result<Option<ExclusionMode>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT path, excluded, mode FROM exclusions")?;
    let rows = stmt.query_map([], |row| {
        let path: String = row.get(0)?;
        let excluded: i64 = row.get(1)?;
        let mode: String = row.get(2)?;
        Ok((path, excluded != 0, mode))
    })?;

    let mut selected: Option<ExclusionMode> = None;
    let mut best_len = 0usize;

    for row in rows {
        let (path, excluded, mode) = row?;
        if !excluded {
            continue;
        }

        if target_path.starts_with(&path) && path.len() >= best_len {
            best_len = path.len();
            selected = Some(match mode.as_str() {
                "ignore" => ExclusionMode::Ignore,
                "read_only" => ExclusionMode::ReadOnly,
                _ => ExclusionMode::Manual,
            });
        }
    }

    Ok(selected)
}

pub fn insert_action_log(db_path: &Path, log: &ActionLog) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        r#"
        INSERT INTO action_logs(timestamp, action, source, destination, reason, model_confidence, rollback_group)
        VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            log.timestamp,
            log.action,
            log.source,
            log.destination,
            log.reason,
            log.model_confidence,
            log.rollback_group
        ],
    )?;
    Ok(())
}

pub fn insert_move_history(db_path: &Path, rollback_group: &str, source: &str, destination: &str) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        "INSERT INTO move_history(rollback_group, source, destination) VALUES(?1, ?2, ?3)",
        params![rollback_group, source, destination],
    )?;
    Ok(())
}

pub fn list_logs(db_path: &Path, limit: usize) -> anyhow::Result<Vec<ActionLog>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT timestamp, action, source, destination, reason, model_confidence, rollback_group FROM action_logs ORDER BY id DESC LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(ActionLog {
            timestamp: row.get(0)?,
            action: row.get(1)?,
            source: row.get(2)?,
            destination: row.get(3)?,
            reason: row.get(4)?,
            model_confidence: row.get(5)?,
            rollback_group: row.get(6)?,
        })
    })?;

    let mut logs = Vec::new();
    for row in rows {
        logs.push(row?);
    }
    Ok(logs)
}

pub fn rollback_moves(db_path: &Path, rollback_group: &str) -> anyhow::Result<usize> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT source, destination FROM move_history WHERE rollback_group = ?1 ORDER BY id DESC",
    )?;

    let rows = stmt.query_map(params![rollback_group], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut restored = 0usize;

    for row in rows {
        let (source, destination) = row?;
        let src_path = Path::new(&source);
        let dst_path = Path::new(&destination);

        if !dst_path.exists() {
            continue;
        }

        if let Some(parent) = src_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if std::fs::rename(dst_path, src_path).is_err() {
            std::fs::copy(dst_path, src_path)?;
            std::fs::remove_file(dst_path)?;
        }
        restored += 1;
    }

    Ok(restored)
}
