use crate::{git::same_path, identity::validate_policy, model::*};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Store {
    pub connection: Connection,
    pub dir: PathBuf,
}
impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let connection = Connection::open(dir.join("repolens.sqlite3"))?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(AppError::new(
                "schema",
                "应用数据版本较新，请使用匹配版本的软件打开。",
            ));
        }
        connection.execute_batch("BEGIN; CREATE TABLE IF NOT EXISTS groups(name TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS entries(id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE, alias TEXT NOT NULL, group_name TEXT REFERENCES groups(name) ON DELETE SET NULL, kind TEXT NOT NULL, policy TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        Ok(Self {
            connection,
            dir: dir.into(),
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut stmt = self
            .connection
            .prepare("SELECT id,path,alias,group_name,kind,policy FROM entries ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, path, alias, group, kind, policy) = row?;
            let policy = serde_json::from_str(&policy).map_err(|_| {
                AppError::new("policy_data", "已保存规则无法读取，未将其当作空规则。")
            })?;
            entries.push(Entry {
                id,
                path,
                alias,
                group,
                kind,
                policy,
            });
        }
        Ok(entries)
    }
    pub fn entry(&self, id: &str) -> Result<Entry> {
        self.entries()?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("entry", "条目不存在或已移出列表。"))
    }
    pub fn add(&self, ctx: &Context) -> Result<Entry> {
        if let Some(entry) = self
            .entries()?
            .into_iter()
            .find(|e| same_path(Path::new(&e.path), Path::new(&ctx.path)))
        {
            return Ok(entry);
        }
        let entry = Entry {
            id: Uuid::new_v4().to_string(),
            path: ctx.path.clone(),
            alias: String::new(),
            group: None,
            kind: ctx.kind.clone(),
            policy: Policy::default(),
        };
        self.connection.execute(
            "INSERT INTO entries VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                entry.id,
                entry.path,
                entry.alias,
                entry.group,
                entry.kind,
                "{\"name\":null,\"email\":null,\"domains\":[]}"
            ],
        )?;
        Ok(entry)
    }
    pub fn update(
        &self,
        id: &str,
        alias: &str,
        group: &Option<String>,
        policy: &Policy,
    ) -> Result<Entry> {
        self.entry(id)?;
        validate_policy(policy)?;
        if (!alias.is_empty() && !valid_text(alias, 256))
            || group.as_ref().is_some_and(|g| !valid_text(g, 100))
        {
            return Err(AppError::new("input", "别名或分组格式无效。"));
        }
        if let Some(group) = group {
            self.add_group(group)?;
        }
        self.connection.execute(
            "UPDATE entries SET alias=?2,group_name=?3,policy=?4 WHERE id=?1",
            params![id, alias, group, serde_json::to_string(policy).unwrap()],
        )?;
        self.entry(id)
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM entries WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn groups(&self) -> Result<Vec<String>> {
        let mut s = self
            .connection
            .prepare("SELECT name FROM groups ORDER BY name")?;
        let groups = s
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(groups)
    }
    pub fn add_group(&self, name: &str) -> Result<()> {
        if !valid_text(name, 100) {
            return Err(AppError::new("input", "分组名称无效。"));
        }
        self.connection
            .execute("INSERT OR IGNORE INTO groups VALUES(?1)", [name])?;
        Ok(())
    }
    pub fn delete_group(&self, name: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM groups WHERE name=?1", [name])?;
        Ok(())
    }
    pub fn setting(&self, key: &str) -> Option<String> {
        self.connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }
    pub fn set_git(&self, value: &str) -> Result<()> {
        self.connection.execute("INSERT INTO settings VALUES('git',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value])?;
        Ok(())
    }
}
