---
title: "AI agent skills"
description: "Download the Xecute skill and complete machine-readable documentation for your coding agent."
---

Give your AI coding agent the same documentation you use. The Xecute skill teaches
it the project structure, MVC/CQRS conventions, request validation, module guards,
integrations, and verification workflow.

## Download the complete skill

[Download the Xecute skill bundle](/skills/xecute.zip)

The ZIP contains one `xecute/` folder:

```text
xecute/
├── SKILL.md                 # Entry point and application conventions
└── references/
    ├── topics.md             # All topics, grouped by task
    ├── full.md              # Complete documentation in one file
    ├── getting-started/     # Tutorials and request flow
    ├── architecture/        # Modules, DI, scopes and lifecycle
    ├── http/                # Controllers, validation, errors and files
    ├── integrations/        # PostgreSQL, JWT, jobs, cache and CQRS
    └── ...                  # CLI, configuration, tests and operations
```

All topic references are included. The agent can read the complete manual or load
only the pages relevant to its task. The bundle is generated from the maintained
docs, so its content stays aligned with this site.

## Install for your agent

1. Download and extract the ZIP.
2. Move the `xecute` folder into your agent's skills directory. For Codex, use
   `~/.codex/skills/xecute/` (or `$CODEX_HOME/skills/xecute/` when configured).
3. Start a new agent session so it can discover the skill.
4. Open your application repository and ask the agent to use the skill.

Other agents can use the Markdown references directly, even if they use a
different skill installation format. Follow that agent's own skill conventions.

Example prompt:

```text
Use $xecute to add a user feature to this application.
Read its existing architecture and the relevant Xecute references first.
Keep controllers thin, put DTOs in dto/, and add HTTP tests for the behavior.
```

To study the whole framework:

```text
Read the Xecute skill and references/full.md.
Explain this project's architecture, then use the relevant topic guides
when implementing the next task. Check the installed APIs before making changes.
```

## Read without installing

| Resource | Best for |
| --- | --- |
| [SKILL.md](/skills/xecute/SKILL.md) | Inspect the agent instructions |
| [Topic index](/llms.txt) | Discover every machine-readable page |
| [Full documentation](/llms-full.txt) | Read all documentation in one request |
| [Bundled reference index](/skills/xecute/references/topics.md) | Browse the skill's included files |

These URLs return plain text or Markdown and do not require JavaScript. Resolve
them relative to the documentation site's base path when hosted in a subdirectory.
Downloading only SKILL.md does not include its references; use the complete ZIP
for an offline skill.

## What the skill helps the agent do

- Follow MVC or CQRS already selected by your application.
- Register module definitions, providers, exports and guarded module routers.
- Keep body/query/path inputs and DTO validation in the right layer.
- Configure optional integrations and lifecycle cleanup explicitly.
- Test route behavior, authorization, persistence boundaries and failures.

The skill supplies guidance, not permission to publish or deploy. Your task and
repository instructions still control what the agent should change. Match the
reference examples to the dependency versions installed in your application.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/docs/handbook/ai-skills.md).
