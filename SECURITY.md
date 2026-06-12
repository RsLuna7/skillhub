# Security Policy

SkillHub indexes and exposes local agent skills. Treat third-party skills like code dependencies.

## Supported Versions

Security fixes target the latest released version.

## Reporting a Vulnerability

Open a private security advisory on GitHub if available, or file an issue with a minimal reproduction that does not expose secrets.

## Current Safety Boundaries

- SkillHub v1 does not execute skill scripts.
- MCP file reads are restricted to indexed skill directories.
- `.env`, hidden files, absolute paths, and path traversal are blocked.
- GitHub installation refuses to overwrite existing skill directories.

## User Guidance

Review third-party skills before installing them. Skills can contain scripts, prompts, and instructions that may influence an agent's behavior.
