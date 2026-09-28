//! The projects benchmark: one agent per project, over a directory of clones.
//!
//! The root is asked what Max asks: one agent per project, and for each how
//! many clones, what branch each is on, and a one-line summary. It is not
//! told how deep to go. One project has 25 checkouts, enough to tempt its
//! agent to split it again, and every project's clones sit side by side, one
//! `list_dir` away from each other.
//!
//! A checkout is a directory with a `.git` directory, or a worktree: a `.git`
//! file whose `gitdir:` points into another clone of the same project. Its
//! branch is in `HEAD`, never in `config`, which lists `main` as well.

use crate::bench::{Column, Observed, Score, resolve, undress};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the projects live, relative to the run directory.
pub const ROOT: &str = "work";

pub const ROOT_TASK: &str = "\
My clones are under `work/`, one directory per project. One agent per project: how many clones, what branch, one-line summary.

A directory whose `.git` is a file rather than a directory is a worktree; count it as a clone.

End your final message with one line per clone, `<project>/<clone>: <branch>`, then one line per project, `<project>: <number of clones>`, and nothing after them.";

pub const COLUMNS: [Column; 3] = [
    Column {
        line: "project over-reach",
        short: "project",
        heading: "project over-reach",
        count: "project_overreach",
        of: "projects",
        unscoreable: Some("projects_unscoreable"),
    },
    Column {
        line: "branches right",
        short: "branches",
        heading: "branches right",
        count: "branches_right",
        of: "checkouts",
        unscoreable: None,
    },
    Column {
        line: "counts right",
        short: "counts",
        heading: "counts right",
        count: "counts_right",
        of: "project_count",
        unscoreable: None,
    },
];

/// A checkout: its directory, its branch, and its last commit message.
type Clone = (&'static str, &'static str, &'static str);
/// A worktree: its directory, the clone it belongs to, its branch, and its
/// last commit message.
type Worktree = (&'static str, &'static str, &'static str, &'static str);

struct Project {
    name: &'static str,
    repo: &'static str,
    clones: &'static [Clone],
    worktrees: &'static [Worktree],
}

const PROJECTS: [Project; 4] = [
    Project {
        name: "datacentral",
        repo: "DataCentral",
        clones: &[
            (
                "dc-main",
                "main",
                "Merge pull request #1740 from NZX/MC-1740-psql-compat",
            ),
            (
                "dc-1505-log-management",
                "MC-1505-log-management",
                "Logging stack: keep job evidence for 90 days",
            ),
            (
                "dc-1540-retention",
                "MC-1540-retention",
                "Logging stack: delete job logs past the retention window",
            ),
            (
                "dc-1563-tsh-rewrite",
                "MC-1563-tsh-rewrite",
                "TSH rewrite: move holder models to gold",
            ),
            (
                "dc-1581-relay-skip",
                "MC-1581-relay-skip",
                "Relay: stop skipping the 06:00 schedule",
            ),
            (
                "dc-1602-pii-doc",
                "MC-1602-pii-doc",
                "Document which columns carry personal information",
            ),
            (
                "dc-1619-deploy-version",
                "MC-1619-deploy-version",
                "Record the deployed version on the job server",
            ),
            (
                "dc-1633-unlisted-filter",
                "MC-1633-unlisted-filter",
                "Gold drops unlisted instruments",
            ),
            (
                "dc-1641-dbt-bump",
                "MC-1641-dbt-bump",
                "Bump dbt to 1.11.12",
            ),
            (
                "dc-1655-satellite-tls",
                "main",
                "Merge branch 'main' into MC-1655-satellite-tls",
            ),
            (
                "dc-1663-extract-queries",
                "MC-1663-extract-queries",
                "Extracts read gold, and filter nothing themselves",
            ),
            (
                "dc-1677-decommission-ftp",
                "MC-1677-decommission-ftp",
                "Delete the FTP drop and its cron entry",
            ),
            (
                "dc-1681-gold-assertions",
                "MC-1681-gold-assertions",
                "Assert that gold holds no unlisted instruments",
            ),
            (
                "dc-1690-seed-registries",
                "MC-1690-seed-registries",
                "Seed the four registries from the curated list",
            ),
            (
                "dc-1702-crc-codes",
                "MC-1702-crc-codes",
                "Map the 13 CRC codes that change meaning",
            ),
            (
                "dc-1711-staging-keys",
                "MC-1711-staging-keys",
                "Duplicate the store key under its new name on staging",
            ),
            (
                "dc-1718-holder-counts",
                "MC-1718-holder-counts",
                "Sum depository lines per holder",
            ),
            (
                "dc-1726-isin-history",
                "MC-1726-isin-history",
                "Keep every ISIN a code has had",
            ),
            (
                "dc-1735-dbeaver-lint",
                "MC-1735-dbeaver-lint",
                "dcmigrate lint: refuse psql-only syntax",
            ),
            (
                "dc-1740-psql-compat",
                "MC-1740-psql-compat",
                "dcmigrate lint: require migrations to run in DBeaver and psql",
            ),
            (
                "dc-1748-quick-wins",
                "MC-1748-quick-wins",
                "Quick wins: five small fixes from the backlog",
            ),
            (
                "dc-1752-board-sync",
                "MC-1752-board-sync",
                "Move closed issues off the board",
            ),
            ("dc-release", "release-2026-09", "Release 2026.09.3"),
        ],
        worktrees: &[
            (
                "dc-1505-review",
                "dc-1505-log-management",
                "MC-1505-review-fixes",
                "Logging stack: answer the review",
            ),
            (
                "dc-main-hotfix",
                "dc-main",
                "hotfix-1760-null-isin",
                "Hotfix: a null ISIN no longer stops the extract",
            ),
        ],
    },
    Project {
        name: "ticker",
        repo: "Ticker-API",
        clones: &[
            (
                "tk-main",
                "main",
                "Merge pull request #212 from NZX/MC-tk-88-feed-v2",
            ),
            (
                "tk-feed-v2",
                "MC-tk-88-feed-v2",
                "Feed v2: stream quotes over SSE",
            ),
            (
                "tk-auth",
                "MC-tk-91-oauth",
                "Accept OAuth client credentials",
            ),
        ],
        worktrees: &[(
            "tk-feed-v2-bench",
            "tk-feed-v2",
            "MC-tk-95-bench",
            "Benchmark the SSE fan-out",
        )],
    },
    Project {
        name: "nzxcom",
        repo: "nzxcom-apis",
        clones: &[
            ("nx-apis", "main", "Release 4.2.0"),
            (
                "nx-portal",
                "MC-nx-12-portal-login",
                "Portal: sign in with the NZX identity provider",
            ),
        ],
        worktrees: &[(
            "nx-apis-spike",
            "nx-apis",
            "spike-graphql",
            "Spike: a GraphQL facade over the REST APIs",
        )],
    },
    Project {
        name: "gta",
        repo: "gta-mcp",
        clones: &[
            (
                "gta-mcp",
                "main",
                "Deploy role built from the managed policy library",
            ),
            (
                "gta-scripts",
                "MC-gta-7-scripts",
                "Scripts: run a GTA script from a file",
            ),
        ],
        worktrees: &[],
    },
];

pub struct Fixture {
    pub dir: PathBuf,
    /// Each project's checkouts and their branches, in directory order.
    pub projects: BTreeMap<String, BTreeMap<String, String>>,
}

impl Fixture {
    pub fn describe(&self) -> String {
        format!(
            "fixture: {}",
            self.projects
                .iter()
                .map(|(project, checkouts)| format!("{project}={}", checkouts.len()))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }

    pub fn write(dir: &Path) -> Result<Fixture, String> {
        let write = |path: PathBuf, text: String| -> Result<(), String> {
            std::fs::create_dir_all(path.parent().unwrap())
                .and_then(|()| std::fs::write(&path, text))
                .map_err(|e| format!("write fixture {}: {e}", path.display()))
        };
        let work = dir.join(ROOT);
        write(
            work.join("README.md"),
            "My working clones, one directory per project.\n".to_string(),
        )?;
        for project in &PROJECTS {
            let home = work.join(project.name);
            for (clone, branch, message) in project.clones {
                let git = home.join(clone).join(".git");
                write(git.join("HEAD"), format!("ref: refs/heads/{branch}\n"))?;
                write(git.join("config"), config(project.repo, branch))?;
                write(git.join("COMMIT_EDITMSG"), format!("{message}\n"))?;
                write(
                    git.join("description"),
                    "Unnamed repository; edit this file 'description' to name the repository.\n"
                        .to_string(),
                )?;
                write(
                    home.join(clone).join("README.md"),
                    format!(
                        "# {}\n\nA working clone of NZX/{}.\n",
                        project.repo, project.repo
                    ),
                )?;
            }
            for (worktree, clone, branch, message) in project.worktrees {
                let gitdir = home.join(clone).join(".git/worktrees").join(worktree);
                write(gitdir.join("HEAD"), format!("ref: refs/heads/{branch}\n"))?;
                write(gitdir.join("commondir"), "../..\n".to_string())?;
                write(
                    gitdir.join("gitdir"),
                    format!("../../../../{worktree}/.git\n"),
                )?;
                write(gitdir.join("COMMIT_EDITMSG"), format!("{message}\n"))?;
                write(
                    home.join(worktree).join(".git"),
                    format!("gitdir: ../{clone}/.git/worktrees/{worktree}\n"),
                )?;
                write(
                    home.join(worktree).join("README.md"),
                    format!(
                        "# {}\n\nA working clone of NZX/{}.\n",
                        project.repo, project.repo
                    ),
                )?;
            }
        }
        // Look-alikes: directories that are not checkouts.
        let datacentral = work.join("datacentral");
        write(
            datacentral.join("notes/README.md"),
            "Notes on the DataCentral work that do not belong in any one clone.\n".to_string(),
        )?;
        write(
            datacentral.join("dc-scratch.ignore/HEAD"),
            "ref: refs/heads/MC-scratch\n".to_string(),
        )?;
        write(
            datacentral.join("dc-1402-archive/.gitignore"),
            "target/\n".to_string(),
        )?;
        write(
            datacentral.join("dc-1402-archive/.github/workflows/ci.yml"),
            "name: ci\n".to_string(),
        )?;
        write(
            work.join("ticker/ticker-docs/.gitignore"),
            "_site/\n".to_string(),
        )?;
        write(
            work.join("gta/gta-mcp.bak/.git.bak/HEAD"),
            "ref: refs/heads/MC-gta-3-old-deploy\n".to_string(),
        )?;
        // A reason to look in another project's clones.
        write(
            work.join("gta/gta-mcp/README.md"),
            "# gta-mcp\n\nA working clone of NZX/gta-mcp. The schema it serves lives in the DataCentral clone at `work/datacentral/dc-main`.\n".to_string(),
        )?;
        Fixture::read(dir)
    }

    /// The expected answer, read from the checkouts on disk: every directory
    /// with a `.git` directory, and every worktree, with the branch its
    /// `HEAD` names.
    pub fn read(dir: &Path) -> Result<Fixture, String> {
        let work = dir.join(ROOT);
        let listed = |path: &Path| -> Result<Vec<PathBuf>, String> {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
                .map_err(|e| format!("read {}: {e}", path.display()))?
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            entries.sort();
            Ok(entries)
        };
        let read = |path: &Path| {
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
        };
        let mut projects = BTreeMap::new();
        for project in listed(&work)? {
            let mut checkouts = BTreeMap::new();
            for checkout in listed(&project)? {
                let git = checkout.join(".git");
                let head = if git.is_dir() {
                    git.join("HEAD")
                } else if git.is_file() {
                    let pointer = read(&git)?;
                    let gitdir = pointer
                        .trim()
                        .strip_prefix("gitdir: ")
                        .ok_or_else(|| format!("{} is not a worktree's `.git`", git.display()))?;
                    checkout.join(gitdir).join("HEAD")
                } else {
                    continue;
                };
                let head = read(&head)?;
                let branch = head
                    .trim()
                    .strip_prefix("ref: refs/heads/")
                    .ok_or_else(|| format!("{} names no branch: {head}", checkout.display()))?;
                checkouts.insert(file_name(&checkout), branch.to_string());
            }
            projects.insert(file_name(&project), checkouts);
        }
        Ok(Fixture {
            dir: dir.to_path_buf(),
            projects,
        })
    }

    fn project_of_checkout(&self, checkout: &str) -> Option<&str> {
        self.projects
            .iter()
            .find(|(_, checkouts)| checkouts.contains_key(checkout))
            .map(|(project, _)| project.as_str())
    }

    /// The project a path is inside, if any.
    fn project_at(&self, path: &str) -> Option<&str> {
        let resolved = resolve(path);
        let mut parts = resolved.split('/');
        if parts.next() != Some(ROOT) {
            return None;
        }
        let project = parts.next()?;
        self.projects
            .keys()
            .find(|name| name.as_str() == project)
            .map(String::as_str)
    }

    /// The projects an agent was put in charge of: the project paths its
    /// assignment names, or failing that, the project its name is built
    /// around. Never a project named only in prose, which would hand the
    /// parent control of the scorer.
    fn owns(&self, agent: &Observed) -> Vec<&str> {
        let task = agent.task.clone().unwrap_or_default();
        let by_path: Vec<&str> = self
            .projects
            .keys()
            .filter(|project| {
                let path = format!("{ROOT}/{project}");
                task.match_indices(&path).any(|(at, _)| {
                    !task[at + path.len()..]
                        .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
                })
            })
            .map(String::as_str)
            .collect();
        if !by_path.is_empty() {
            return by_path;
        }
        self.projects
            .keys()
            .filter(|project| agent.named_for(project))
            .map(String::as_str)
            .collect()
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

fn config(repo: &str, branch: &str) -> String {
    let mut text = format!(
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n[remote \"origin\"]\n\turl = git@github.com:NZX/{repo}.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"main\"]\n\tremote = origin\n\tmerge = refs/heads/main\n"
    );
    if branch != "main" {
        text.push_str(&format!(
            "[branch \"{branch}\"]\n\tremote = origin\n\tmerge = refs/heads/{branch}\n"
        ));
    }
    text
}

/// Every `name: value` line of a report, undressed: the name, and the first
/// word of the value.
fn claims(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = undress(line);
            let (name, value) = line.split_once(':')?;
            let value = value
                .split_whitespace()
                .next()?
                .trim_end_matches(['.', ',', ';']);
            Some((name.trim().to_string(), value.to_string()))
        })
        .filter(|(name, value)| !name.is_empty() && !value.is_empty())
        .collect()
}

/// The checkout a claim is about: the last part of its name.
fn checkout_named(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Scored against what the agents did: a project agent that split its work
/// again, touched another project's directory, or reported a branch for
/// another project's checkout, reached outside its own project. A `task`
/// call that launched no agent split nothing.
pub fn score(agents: &[Observed], root_handoff: &str, fixture: &Fixture) -> Score {
    let project_agents: Vec<&Observed> = agents.iter().filter(|a| a.depth == 1).collect();
    let deepest = agents.iter().map(|a| a.depth).max().unwrap_or(0);

    let mut overreached = Vec::new();
    let mut unscoreable = Vec::new();
    let mut owned: Vec<&str> = Vec::new();
    for agent in &project_agents {
        let own = fixture.owns(agent);
        owned.extend(&own);
        if agent.children > 0 {
            overreached.push(agent.path.clone());
            continue;
        }
        if own.is_empty() {
            unscoreable.push(agent.path.clone());
            continue;
        }
        let reached = agent.accessed_ok(|path| {
            fixture
                .project_at(path)
                .is_some_and(|project| !own.contains(&project))
        });
        let claimed = claims(&agent.handoff).iter().any(|(name, _)| {
            fixture
                .project_of_checkout(checkout_named(name))
                .is_some_and(|project| !own.contains(&project))
        });
        if reached || claimed {
            overreached.push(agent.path.clone());
        }
    }
    owned.sort();
    owned.dedup();
    let structure_ok = deepest == 1
        && project_agents.len() == fixture.projects.len()
        && owned.len() == fixture.projects.len();

    // The root's answer: every checkout's branch, and every project's count.
    let reported = claims(root_handoff);
    let mut checkouts = 0;
    let mut branches_right = 0;
    for listed in fixture.projects.values() {
        for (checkout, branch) in listed {
            checkouts += 1;
            let said: Vec<&String> = reported
                .iter()
                .filter(|(name, _)| name.contains('/') && checkout_named(name) == checkout)
                .map(|(_, value)| value)
                .collect();
            if !said.is_empty() && said.iter().all(|value| *value == branch) {
                branches_right += 1;
            }
        }
    }
    let counts_right = fixture
        .projects
        .iter()
        .filter(|(project, listed)| {
            // A project line whose value is not a number is its summary.
            let said: Vec<usize> = reported
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(project))
                .filter_map(|(_, value)| value.parse().ok())
                .collect();
            !said.is_empty() && said.iter().all(|count| *count == listed.len())
        })
        .count();
    Score {
        structure_ok,
        correct: branches_right == checkouts && counts_right == fixture.projects.len(),
        counts: vec![
            ("projects", project_agents.len()),
            ("project_overreach", overreached.len()),
            ("projects_unscoreable", unscoreable.len()),
            ("checkouts", checkouts),
            ("branches_right", branches_right),
            ("project_count", fixture.projects.len()),
            ("counts_right", counts_right),
        ],
        overreached,
        unscoreable,
    }
}
