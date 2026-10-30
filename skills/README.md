# Agent skills

These are [Agent Skills](https://agentskills.io): folders with a `SKILL.md`
that a coding agent such as Claude Code or Codex loads when a task matches
the skill's description.

| Skill | Use it to |
|---|---|
| [`sley`](sley/SKILL.md) | Write, test and submit Sley programs with `sley-agent`, in the typed Python dialect |
| [`sley-embed`](sley-embed/SKILL.md) | Run Sley inside a Rust host or a WebAssembly build with `sley_agent::embed` |

Both describe the 2.1.7 release. Their examples were run against the
released binaries and the `v2.1.7` tag.

## Install

Copy or symlink a skill folder into your agent's skills directory. For
Claude Code, that is `~/.claude/skills/` for every project or
`.claude/skills/` inside one project. From a checkout of this repository:

```sh
mkdir -p ~/.claude/skills
cp -r skills/sley skills/sley-embed ~/.claude/skills/
```

Use `ln -s "$PWD/skills/sley" ~/.claude/skills/sley` instead to follow the
checkout as it updates. Other agents that read Agent Skills take the same
folders; see the agent's documentation for its skills directory. An agent
without skill support can read a `SKILL.md` as plain instructions.
