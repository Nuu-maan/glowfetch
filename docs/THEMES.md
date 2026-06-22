# Themes

glowfetch ships with five built in themes. Select one with the `theme` key in the config file or the `--theme` flag.

| Theme | Accent | Secondary | Notes |
| --- | --- | --- | --- |
| windows | `#00AEEF` | `#785AFF` | Default. Blue to violet, matches the Windows flag. |
| matrix | `#00FF66` | `#00AA33` | Green on dark, terminal classic, pale green text. |
| dracula | `#BD93F9` | `#FF79C6` | Purple to pink, a popular dark palette. |
| nord | `#88C0D0` | `#5E81AC` | Frost blue, calm and muted. |
| amber | `#FFB000` | `#FF7000` | Warm orange, retro CRT feel. |

## Custom colors

You do not need a new theme to change colors. Start from any theme and override the accents:

```toml
theme = "nord"
accent = "#8fbcbb"
accent2 = "#5e81ac"
```

The accent drives gauges, titles, the network graph, and the logo gradient start. The secondary accent drives the user name, the system title, and the logo gradient end.

## Try them quickly

```powershell
glowfetch --theme matrix
glowfetch --theme dracula
glowfetch --theme nord --no-logo
glowfetch --theme amber --once
```
