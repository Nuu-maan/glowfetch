# Configuration reference

glowfetch reads a TOML file from `%APPDATA%\glowfetch\glowfetch.toml` when present. You can also point at a specific file with `--config <PATH>`. Generate a starter file with `glowfetch --gen-config`.

Command line flags take priority over the config file, and the config file takes priority over the built in defaults.

## Keys

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `theme` | string | `windows` | One of windows, matrix, dracula, nord, amber. |
| `accent` | string | theme value | Override the primary accent. Hex such as `#00AEEF` or `r,g,b`. |
| `accent2` | string | theme value | Override the secondary accent. |
| `text` | string | theme value | Override the info text color. |
| `show_logo` | bool | `true` | Show or hide the ASCII logo. |
| `fancy` | string | `auto` | `auto`, `on`, or `off`. Auto enables icons inside Windows Terminal. |

## Sections

The `[sections]` table toggles individual panels. Any panel set to false is removed from the layout.

```toml
[sections]
cpu = true
ram = true
disk = true
net = true
palette = true
```

## Color formats

- Hex: `#RRGGBB`, for example `#785AFF`.
- RGB triple: `r,g,b`, for example `120,90,255`.

Invalid values fall back to the active theme.

## Example

```toml
theme = "dracula"
accent = "#bd93f9"
accent2 = "#ff79c6"
show_logo = true
fancy = "auto"

[sections]
cpu = true
ram = true
disk = true
net = false
palette = true
```
