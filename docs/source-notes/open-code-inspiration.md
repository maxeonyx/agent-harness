# OpenCode fork as inspiration

This harness should take strong inspiration from my OpenCode fork, albeit not strict parity.

If this harness has a similar feature or behavior, then the specific behavior should match my fork of OpenCode as closely as makes sense.

## The fork's features

The features I added to my fork of OpenCode, as of its last release, `v1.14.22-max.21`.

### Releases and updates

- **fork-setup**: removed upstream's release workflows, so the fork had its own release infrastructure.
- **fork-version-handling**: builds take their version from the exact release tag, e.g. `1.14.22-max.10`.
- **fork-auto-update**: fork builds check the fork's own releases, and install an update in place from the lobby screen.

### GitHub Copilot

- **copilot-provider**: a Copilot provider with shared discovery and auth, live model loading, billing classification and Claude-native routing.
- **copilot-quota**: the TUI header and sidebar show usage inline: premium requests for Copilot, dollar cost for other providers.
- **request-metadata-headers**: Copilot requests carry `x-opencode-*` headers naming the session, request, agent and small-model traffic.

### Sessions

- **ephemeral-sessions**: headless runs are in-memory by default; `--no-ephemeral` persists them.
- **session-resume**: `--continue` picks the latest session in the current directory; `--session <id>` brings a session into the current directory; headless resume copies the history instead of changing it.
- **process-session-binding**: a table mapping each running process to its session, so external tools can find a session and reattach.
- **session-info-command**: a Session info dialog with the session's metadata and copy actions.
- **subagent-sort-recency**: sibling subagent sessions are sorted by most recent activity, and navigation stops at the ends.

### Context and compaction

- **context-introspection**: captures the exact context sent to the model on each request, as a first-class object.
- **context-footer-estimate**: a `~XK/YK/ZK` footer showing estimated context use, replaced by the actual figure when the response arrives.
- **context-dump**: a command that copies the captured context as markdown: system text, messages and tool schemas.
- **pre-compaction-warning**: the model is warned when context passes a threshold (default 80%), so it can plan a handover.
- **skip-terminal-compaction**: no compaction after an overflow when the turn is finished anyway.
- **handover-compaction**: the active agent writes its own compaction summary through a handover tool, instead of a separate compaction agent.
- **unwrap-system-prompt**: system prompts with no hard-wrapped lines, so they lead by example.

### Agents, tools and skills

- **agents-md-ordering**: AGENTS.md discovery reaches the git superproject root, in a fixed order across workspace and submodule roots.
- **task-resume-split**: separate Task (new subagent) and Resume (existing subagent) tools.
- **task-abort-preserves-id**: an aborted subagent's error keeps its `task_id`, so it can be resumed.
- **task-abort-enrichment**: an aborted subagent reports how long it ran, how many tools it used, and whether it had started working.
- **skill-availability**: SKILL.md front matter can limit which agents see a skill.
- **blank-agent**: `blank: true` runs an agent with no injected context, to compare against the model's baseline.
- **read-show-user**: a `show_user` option on the Read tool, so the user sees the file the agent is reading.
- **permission-timeout**: a permission prompt is denied after a configurable timeout, so an absent user does not stall the agent.
- **remote-mcp-auth-command**: a remote MCP server's bearer token can come from a command.

### TUI

- **tui-header**: header and sidebar accents per project and per session.
- **prompt-directory**: a `directory:branch` line above the prompt.
- **compact-ui**: a denser layout, with half-height padding rows.

### Fixes

- **theme-detection-fix**: light/dark terminal detection always said dark, because the luminance was divided by 255 twice.
- **title-generation-fix**: session titles were never generated, because the user message content was passed as a string.
- **mcp-auth-timeout**: `opencode mcp auth` hung for about 135 seconds on WSL2, because a port check had no timeout.
- **system-timezone-locale**: dates use the locale from `LC_ALL` or `LANG`, so New Zealand times show `NZST`, not `GMT+12`.

### Dropped before the last release

- **compaction-system-prompt**: the compaction summariser saw the original agent's instructions as reference material.
- **remove-compaction-followup**: after compaction, a minimal synthetic "continue" replaced an instructional message that read as the user talking.
- **tool-triggered-compaction**: a tool result could ask for compaction on the next turn.
