# Security policy

## Reporting a vulnerability

Please report security issues privately through
[GitHub's private vulnerability reporting](https://github.com/glauberlima/open-scrantic/security/advisories/new)
instead of opening a public issue. Include the version (`open-scrantic --version`), your operating
system and the steps to reproduce.

## Scope

Open Scrantic runs locally and reads three files from the directory of its executable. It makes no
network connections and contains `unsafe` code only in the Windows screensaver build (`src/windows_ui.rs`, Win32 calls). Relevant reports include, for example, a crash or
excessive memory use triggered by a malformed `SCRANTIC.SCR`, `RESOURCE.MAP`, `RESOURCE.001` or
`SCRANTIC.INI`, or a problem in the release workflows.

## Supported versions

Only the latest release receives fixes.
