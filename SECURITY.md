# Security policy

## Reporting a vulnerability

Please report security issues privately through
[GitHub's private vulnerability reporting](https://github.com/glauberlima/open-scrantic/security/advisories/new)
instead of opening a public issue. Include the version (`open-scrantic --version`, or the Details tab of the `.scr` properties), your operating
system and the steps to reproduce.

## Scope

Open Scrantic runs locally and reads three files from the directory of its executable. It makes no
network connections and contains `unsafe` code only in the Windows screensaver build (`src/windows_ui.rs`, Win32 calls). Relevant reports include, for example, a crash or
excessive memory use triggered by a malformed `SCRANTIC.SCR`, `RESOURCE.MAP`, `RESOURCE.001` or
`SCRANTIC.INI`, or a problem in the release workflows. The loaders reject malformed data files with an error and cap the
memory, time and nesting they spend on them (see "Deliberate differences" in `docs/ENGINE.md`); a file that gets
past those caps is a bug worth reporting.

Out of scope: anything that needs write access to the folder the program runs from (a replaced executable or DLL
has the same power as the user), and advisories in dependencies that the program does not reach, such as the
Wayland-only window frame code pulled in by `winit`. Run the program from a folder of its own, since it trusts
the files it finds there.

## Supported versions

Only the latest release receives fixes.
