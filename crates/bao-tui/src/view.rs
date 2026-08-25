//! View models: typed session data and its pure derivations, built from the
//! daemon's `SessionMeta` facts. No I/O, no rendering — components and the
//! overview render these as-is.

use bao_core::{
    alert::Alert,
    types::{SessionId, SessionMeta, Status},
};
use ratatui::style::{Modifier, Style};

use crate::{signal, theme};

/// Where a session belongs on the overview — derived from daemon facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    NeedsYou,
    Working,
    Done,
    Other,
}

impl Group {
    pub fn order(self) -> u8 {
        match self {
            Group::NeedsYou => 0,
            Group::Working => 1,
            Group::Done => 2,
            Group::Other => 3,
        }
    }

    pub fn of(status: Status, alert: Option<Alert>, waiting: bool) -> Group {
        if waiting {
            return Group::NeedsYou;
        }
        match alert {
            Some(Alert::Damaged | Alert::Errored(_) | Alert::Interrupted | Alert::Idle(_)) => {
                Group::NeedsYou
            }
            Some(Alert::Done) => Group::Done,
            None => match status {
                Status::Running | Status::Preparing | Status::Starting => Group::Working,
                _ => Group::Other,
            },
        }
    }
}

/// One session row, modelled from its typed meta.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: SessionId,
    pub name: String,
    pub command: String,
    pub status: Status,
    pub age_secs: u64,
    pub idle_secs: u64,
    pub alert: Option<Alert>,
    pub waiting: bool,
    pub group: Group,
    pub meta: SessionMeta,
}

impl Row {
    pub fn from_meta(m: &SessionMeta) -> Row {
        let waiting = m.waiting_for_input == Some(true);
        Row {
            id: m.id.clone(),
            name: m.name.clone().unwrap_or_default(),
            command: m.command.clone(),
            status: m.status,
            age_secs: m.age_secs,
            idle_secs: m.idle_secs,
            alert: m.alert,
            waiting,
            group: Group::of(m.status, m.alert, waiting),
            meta: m.clone(),
        }
    }

    pub fn signal(&self) -> signal::Signal {
        signal::signal(self.status, self.alert, self.waiting, self.idle_secs)
    }

    pub fn glyph(&self) -> char {
        self.signal().glyph
    }

    pub fn style(&self) -> Style {
        self.signal().style
    }

    pub fn status_text(&self) -> String {
        self.signal().text
    }

    pub fn rail_word(&self) -> String {
        if self.signal().waiting_visible {
            return "waiting".to_string();
        }
        match self.alert {
            Some(Alert::Damaged) => "damaged".to_string(),
            Some(Alert::Errored(_)) => "errored".to_string(),
            Some(Alert::Interrupted) => "interrupted".to_string(),
            Some(Alert::Idle(s)) => format!("idle {}", signal::fmt_age(s)),
            Some(Alert::Done) => "done".to_string(),
            None => match self.status {
                Status::Preparing => "preparing".to_string(),
                Status::Starting => "starting".to_string(),
                _ => "running".to_string(),
            },
        }
    }

    pub fn edge_glyph(&self) -> (char, Style) {
        let g = theme::glyphs();
        let p = theme::palette();
        if self.signal().waiting_visible {
            return (g.full, Style::default().fg(p.waiting));
        }
        match self.alert {
            Some(Alert::Damaged) => (g.full, Style::default().fg(p.damaged)),
            Some(Alert::Errored(_)) => (g.full, Style::default().fg(p.errored)),
            Some(Alert::Interrupted) => (g.full, Style::default().fg(p.interrupted)),
            Some(Alert::Idle(_)) => (g.half, Style::default().fg(p.idle)),
            Some(Alert::Done) => (g.dot, Style::default().fg(p.done)),
            None => match self.status {
                Status::Preparing => (g.hollow, Style::default().fg(p.dim)),
                Status::Starting => (g.hollow, Style::default().fg(p.accent)),
                _ => (g.thin, Style::default().fg(p.healthy)),
            },
        }
    }

    pub fn action_hint(&self) -> (String, Style) {
        let p = theme::palette();
        let a = theme::glyphs().arrow;
        let dim = Style::default().fg(p.dim);
        if self.signal().waiting_visible {
            return (
                format!("{a} waiting for you"),
                Style::default().fg(p.waiting).add_modifier(Modifier::BOLD),
            );
        }
        match self.alert {
            Some(Alert::Damaged) => (
                format!("{a} needs a human — meta unreadable"),
                Style::default().fg(p.damaged).add_modifier(Modifier::BOLD),
            ),
            Some(Alert::Errored(c)) => (
                format!("{a} exited with code {c}"),
                Style::default().fg(p.errored).add_modifier(Modifier::BOLD),
            ),
            Some(Alert::Interrupted) => (
                format!("{a} needs resume (r) or remove (d)"),
                Style::default()
                    .fg(p.interrupted)
                    .add_modifier(Modifier::BOLD),
            ),
            Some(Alert::Idle(s)) => (
                format!("{a} idle {} — watching", signal::fmt_age(s)),
                Style::default().fg(p.idle),
            ),
            Some(Alert::Done) => (format!("{a} finished"), dim),
            None => match self.status {
                Status::Preparing => (
                    format!("{a} preparing — building the sandbox"),
                    Style::default().fg(p.dim),
                ),
                Status::Starting => (
                    format!("{a} starting — waiting for first output"),
                    Style::default().fg(p.accent),
                ),
                _ => (format!("{a} nothing needs your action"), dim),
            },
        }
    }

    pub fn inner_rank(&self) -> u8 {
        self.signal().rank
    }
}

/// Sort sessions by signal group, severity, then age.
pub fn sort_rows(rows: &mut [Row]) {
    rows.sort_by_key(|r| (r.group.order(), r.inner_rank(), r.age_secs));
}

/// The sidebar label for sessions launched at a raw directory — honest about
/// having no registered target.
pub const UNGROUPED: &str = "unassigned";

/// One workspace's slice of the sidebar: its sessions in severity order,
/// plus whether any of them needs the human.
pub struct WorkspaceGroup {
    pub name: String,
    pub needs_attention: bool,
    pub rows: Vec<Row>,
}

/// Group sessions by their launch target: named workspaces alphabetically,
/// unassigned last. Order within a group is whatever order the rows arrived
/// in (the severity sort). Pure derivation — views render this as-is.
pub fn group_rows(rows: &[Row]) -> Vec<WorkspaceGroup> {
    let mut groups: Vec<WorkspaceGroup> = Vec::new();
    for row in rows {
        let name = row
            .meta
            .workspace
            .clone()
            .unwrap_or_else(|| UNGROUPED.to_string());
        match groups.iter_mut().find(|g| g.name == name) {
            Some(g) => g.rows.push(row.clone()),
            None => groups.push(WorkspaceGroup {
                name,
                needs_attention: false,
                rows: vec![row.clone()],
            }),
        }
    }
    for g in &mut groups {
        g.needs_attention = g.rows.iter().any(|r| r.group == Group::NeedsYou);
    }
    groups.sort_by(|a, b| {
        let un = |g: &WorkspaceGroup| g.name == UNGROUPED;
        match (un(a), un(b)) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.name.cmp(&b.name),
        }
    });
    groups
}

/// One tab in the top bar: an open terminal, echoed as glyph + title.
#[derive(Debug, Clone)]
pub struct TabView {
    pub title: String,
    pub glyph: char,
    pub style: Style,
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bao_core::{
        alert::AlertInput,
        sandbox::{SandboxKind, WorkingCopy},
        types::{Command, Hostname, SessionMeta, Status},
    };

    use super::*;

    fn meta(status: Status, idle_secs: u64) -> SessionMeta {
        let now = 1_000_000u64;
        SessionMeta {
            id: SessionId::from_str("abc12345").unwrap(),
            name: Some("worker".to_string()),
            command: "pi".to_string(),
            args: Command::from_args(vec!["pi".to_string()]),
            cwd: "/tmp".into(),
            working_copy: WorkingCopy {
                kind: SandboxKind::Worktree,
                repo: None,
                branch: Some("bao-abc12345".into()),
                path: "/tmp/tree".into(),
            },
            workspace: None,
            created: now - 60_000,
            host: Hostname::parse("localhost").unwrap(),
            status,
            last_activity: now - idle_secs * 1000,
            last_output: "hi".into(),
            alert: AlertInput { status, idle_secs }.alert(),
            waiting_for_input: None,
            idle_secs,
            age_secs: 60,
        }
    }

    fn named(id: &str, name: &str, status: Status) -> Row {
        let mut m = meta(status, 5);
        m.id = SessionId::from_str(id).unwrap();
        m.name = Some(name.to_string());
        Row::from_meta(&m)
    }

    #[test]
    fn from_meta_derives_group_and_glyph() {
        assert_eq!(
            Row::from_meta(&meta(Status::Running, 5)).group,
            Group::Working
        );
        assert_eq!(Row::from_meta(&meta(Status::Running, 5)).glyph(), '●');
        assert_eq!(
            Row::from_meta(&meta(Status::Running, 200)).glyph(),
            '…',
            "idle past threshold"
        );
        assert_eq!(
            Row::from_meta(&meta(Status::Exited(Some(1)), 0)).glyph(),
            '✕'
        );
        assert_eq!(
            Row::from_meta(&meta(Status::Exited(Some(0)), 0)).glyph(),
            '✓'
        );
        assert_eq!(Row::from_meta(&meta(Status::Interrupted, 0)).glyph(), '⏸');
    }

    #[test]
    fn waiting_agent_outranks_idle() {
        let mut m = meta(Status::Running, 200);
        m.waiting_for_input = Some(true);
        let r = Row::from_meta(&m);
        assert!(r.waiting);
        assert_eq!(r.group, Group::NeedsYou);
        assert_eq!(r.glyph(), '◉');
    }

    #[test]
    fn boot_states_group_as_working() {
        let preparing = Row::from_meta(&meta(Status::Preparing, 0));
        assert_eq!(preparing.group, Group::Working);
        assert_eq!(preparing.rail_word(), "preparing");

        let starting = Row::from_meta(&meta(Status::Starting, 0));
        assert_eq!(starting.group, Group::Working);
        assert_eq!(starting.rail_word(), "starting");
    }

    #[test]
    fn sort_orders_by_group_severity_age() {
        let mut rows = vec![
            named("a", "done", Status::Exited(Some(0))),
            named("b", "errored", Status::Exited(Some(2))),
            named("c", "running", Status::Running),
            named("d", "interrupted", Status::Interrupted),
        ];
        sort_rows(&mut rows);
        // damaged/errored first, then interrupted, then working, then done.
        assert_eq!(rows[0].name, "errored");
        assert_eq!(rows[1].name, "interrupted");
        assert_eq!(rows[2].name, "running");
        assert_eq!(rows[3].name, "done");
    }

    fn in_workspace(mut m: SessionMeta, ws: &str) -> Row {
        m.workspace = Some(ws.to_string());
        Row::from_meta(&m)
    }

    #[test]
    fn groups_by_workspace_alphabetical_unassigned_last() {
        let mut m = meta(Status::Running, 5);
        let rows = vec![
            in_workspace(m.clone(), "zeta"),
            {
                m.id = SessionId::from_str("abc12346").unwrap();
                in_workspace(m.clone(), "alpha")
            },
            {
                m.id = SessionId::from_str("abc12347").unwrap();
                m.workspace = None;
                Row::from_meta(&m)
            },
        ];
        let groups = group_rows(&rows);
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta", UNGROUPED]);
        assert!(!groups[0].needs_attention);
    }

    #[test]
    fn group_needs_attention_when_any_row_does() {
        let mut m = meta(Status::Interrupted, 0);
        let rows = vec![in_workspace(m.clone(), "app")];
        m.id = SessionId::from_str("abc12348").unwrap();
        let groups = group_rows(&rows);
        assert!(groups[0].needs_attention);
    }
}
