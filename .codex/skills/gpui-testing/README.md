# GPUI Testing Skill

A reusable agent skill for planning, implementing, reviewing, and debugging tests in Rust applications built with GPUI.

## Contents

- `SKILL.md`: trigger description, decision framework, implementation workflow, guardrails, verification, and output contract
- `references/recipes.md`: reusable test patterns and examples

## Installation

Copy the `gpui-testing-skill` directory into the skills directory used by your agent environment, preserving the directory structure.

The skill deliberately avoids pinning its guidance to one GPUI release. It instructs the agent to resolve and inspect the repository's exact GPUI dependency before generating code.
