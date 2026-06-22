use std::collections::VecDeque;
use std::io;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use serde::Deserialize;
use sysinfo::{Disks, Networks, System};
use wmi::{COMLibrary, WMIConnection};

const HIST: usize = 120;

type Rgb = (u8, u8, u8);

// ---------- theme / settings ----------

#[derive(Clone)]
struct Theme {
    accent: Rgb,
    accent2: Rgb,
    text: Rgb,
    dim: Rgb,
    track: Rgb,
    logo_a: Rgb,
    logo_b: Rgb,
}

#[derive(Clone)]
struct Sections {
    cpu: bool,
    ram: bool,
    disk: bool,
    net: bool,
    palette: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Sections { cpu: true, ram: true, disk: true, net: true, palette: true }
    }
}

#[derive(Clone)]
struct Settings {
    theme: Theme,
    sections: Sections,
    show_logo: bool,
    fancy: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: preset("windows"),
            sections: Sections::default(),
            show_logo: true,
            fancy: std::env::var("WT_SESSION").is_ok(),
        }
    }
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();
fn st() -> &'static Settings {
    SETTINGS.get_or_init(Settings::default)
}

fn col(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

fn accent() -> Color {
    col(st().theme.accent)
}
fn accent2() -> Color {
    col(st().theme.accent2)
}
fn dim() -> Color {
    col(st().theme.dim)
}
fn white() -> Color {
    col(st().theme.text)
}
fn track() -> Color {
    col(st().theme.track)
}

fn preset(name: &str) -> Theme {
    let base = Theme {
        accent: (0, 174, 239),
        accent2: (120, 90, 255),
        text: (235, 238, 245),
        dim: (110, 110, 125),
        track: (30, 32, 40),
        logo_a: (0, 174, 239),
        logo_b: (120, 90, 255),
    };
    match name.to_lowercase().as_str() {
        "matrix" => Theme {
            accent: (0, 255, 102),
            accent2: (0, 170, 51),
            text: (200, 255, 200),
            logo_a: (0, 255, 102),
            logo_b: (0, 120, 40),
            ..base
        },
        "dracula" => Theme {
            accent: (189, 147, 249),
            accent2: (255, 121, 198),
            logo_a: (189, 147, 249),
            logo_b: (255, 121, 198),
            ..base
        },
        "nord" => Theme {
            accent: (136, 192, 208),
            accent2: (94, 129, 172),
            logo_a: (136, 192, 208),
            logo_b: (94, 129, 172),
            ..base
        },
        "amber" => Theme {
            accent: (255, 176, 0),
            accent2: (255, 112, 0),
            logo_a: (255, 176, 0),
            logo_b: (255, 112, 0),
            ..base
        },
        _ => base, // "windows" / unknown
    }
}

fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            return Some((
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ));
        }
    }
    let p: Vec<&str> = s.split(',').collect();
    if p.len() == 3 {
        return Some((p[0].trim().parse().ok()?, p[1].trim().parse().ok()?, p[2].trim().parse().ok()?));
    }
    None
}

// ---------- config file ----------

#[derive(Deserialize, Default)]
struct SectionCfg {
    cpu: Option<bool>,
    ram: Option<bool>,
    disk: Option<bool>,
    net: Option<bool>,
    palette: Option<bool>,
}

#[derive(Deserialize, Default)]
struct Config {
    theme: Option<String>,
    accent: Option<String>,
    accent2: Option<String>,
    text: Option<String>,
    show_logo: Option<bool>,
    fancy: Option<String>, // auto | on | off
    sections: Option<SectionCfg>,
}

const SAMPLE_CONFIG: &str = "\
# glowfetch configuration
theme = \"windows\"        # windows | matrix | dracula | nord | amber
# accent  = \"#00AEEF\"     # override theme accent (hex or \"r,g,b\")
# accent2 = \"#785AFF\"
show_logo = true
fancy = \"auto\"           # auto (Windows Terminal) | on | off

[sections]
cpu = true
ram = true
disk = true
net = true
palette = true
";

fn default_config_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&base).join("glowfetch").join("glowfetch.toml")
}

fn build_settings(cfg: Config, theme_override: Option<String>, fancy_override: Option<bool>, logo_override: Option<bool>) -> Settings {
    let theme_name = theme_override.or(cfg.theme).unwrap_or_else(|| "windows".into());
    let mut theme = preset(&theme_name);
    if let Some(c) = cfg.accent.as_deref().and_then(parse_color) {
        theme.accent = c;
        theme.logo_a = c;
    }
    if let Some(c) = cfg.accent2.as_deref().and_then(parse_color) {
        theme.accent2 = c;
        theme.logo_b = c;
    }
    if let Some(c) = cfg.text.as_deref().and_then(parse_color) {
        theme.text = c;
    }

    let fancy = fancy_override.unwrap_or_else(|| match cfg.fancy.as_deref() {
        Some("on") => true,
        Some("off") => false,
        _ => std::env::var("WT_SESSION").is_ok(),
    });

    let sc = cfg.sections.unwrap_or_default();
    let sections = Sections {
        cpu: sc.cpu.unwrap_or(true),
        ram: sc.ram.unwrap_or(true),
        disk: sc.disk.unwrap_or(true),
        net: sc.net.unwrap_or(true),
        palette: sc.palette.unwrap_or(true),
    };

    Settings {
        theme,
        sections,
        show_logo: logo_override.or(cfg.show_logo).unwrap_or(true),
        fancy,
    }
}

const LOGO: &[&str] = &[
    "      ▟███████▙ ▟███████▙",
    "      █████████ █████████",
    "      █████████ █████████",
    "      █████████ █████████",
    "      ▜███████▛ ▜███████▛",
    "                         ",
    "      ▟███████▙ ▟███████▙",
    "      █████████ █████████",
    "      █████████ █████████",
    "      █████████ █████████",
    "      ▜███████▛ ▜███████▛",
];

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> Color {
    let (r, g, bl) = lerp_rgb(a, b, t);
    Color::Rgb(r, g, bl)
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f64 + (b.0 as f64 - a.0 as f64) * t) as u8,
        (a.1 as f64 + (b.1 as f64 - a.1 as f64) * t) as u8,
        (a.2 as f64 + (b.2 as f64 - a.2 as f64) * t) as u8,
    )
}

fn load_color(p: f64) -> Color {
    if p < 0.6 {
        lerp((40, 200, 120), (230, 200, 60), p / 0.6)
    } else {
        lerp((230, 200, 60), (240, 70, 70), (p - 0.6) / 0.4)
    }
}

// ---------- WMI ----------

#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController")]
#[serde(rename_all = "PascalCase")]
struct VideoController {
    name: Option<String>,
    current_horizontal_resolution: Option<u32>,
    current_vertical_resolution: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_Battery")]
#[serde(rename_all = "PascalCase")]
struct Battery {
    estimated_charge_remaining: Option<u16>,
}

// ---------- App ----------

struct App {
    sys: System,
    disks: Disks,
    networks: Networks,
    wmi: Option<WMIConnection>,

    host: String,
    os: String,
    kernel: String,
    cpu_brand: String,
    arch: String,
    gpu: String,
    resolution: String,

    disk_used: u64,
    disk_total: u64,
    net_down: f64,
    net_up: f64,
    battery: Option<u16>,

    cpu_hist: VecDeque<u64>,
    net_hist: VecDeque<u64>,
}

impl App {
    fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown CPU".into());

        let wmi = COMLibrary::new().ok().and_then(|com| WMIConnection::new(com).ok());

        let (mut gpu, mut resolution) = ("Unknown".to_string(), "-".to_string());
        if let Some(conn) = &wmi {
            if let Ok(vcs) = conn.query::<VideoController>() {
                if let Some(v) = vcs.into_iter().find(|v| v.name.is_some()) {
                    gpu = v.name.unwrap_or_else(|| "Unknown".into());
                    if let (Some(w), Some(h)) =
                        (v.current_horizontal_resolution, v.current_vertical_resolution)
                    {
                        if w > 0 && h > 0 {
                            resolution = format!("{w}x{h}");
                        }
                    }
                }
            }
        }

        let mut app = App {
            host: System::host_name().unwrap_or_else(|| "unknown".into()),
            os: format!(
                "{} {}",
                System::name().unwrap_or_else(|| "Windows".into()),
                System::os_version().unwrap_or_default()
            ),
            kernel: System::kernel_version().unwrap_or_else(|| "-".into()),
            arch: System::cpu_arch(),
            cpu_brand,
            gpu,
            resolution,
            disk_used: 0,
            disk_total: 0,
            net_down: 0.0,
            net_up: 0.0,
            battery: None,
            cpu_hist: VecDeque::from(vec![0u64; HIST]),
            net_hist: VecDeque::from(vec![0u64; HIST]),
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            wmi,
        };
        app.refresh_disk();
        app
    }

    fn refresh_disk(&mut self) {
        let mut best: Option<(u64, u64)> = None;
        for d in self.disks.list() {
            let mount = d.mount_point().to_string_lossy().to_uppercase();
            let total = d.total_space();
            let used = total.saturating_sub(d.available_space());
            if mount.starts_with("C:") {
                best = Some((used, total));
                break;
            }
            if best.map_or(true, |(_, t)| total > t) {
                best = Some((used, total));
            }
        }
        if let Some((u, t)) = best {
            self.disk_used = u;
            self.disk_total = t;
        }
    }

    fn tick(&mut self, dt: f64) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        self.networks.refresh(true);
        let (mut rx, mut tx) = (0u64, 0u64);
        for (_n, data) in self.networks.iter() {
            rx += data.received();
            tx += data.transmitted();
        }
        let dt = dt.max(0.1);
        self.net_down = rx as f64 / dt;
        self.net_up = tx as f64 / dt;

        if let Some(conn) = &self.wmi {
            if let Ok(b) = conn.query::<Battery>() {
                self.battery = b.into_iter().find_map(|x| x.estimated_charge_remaining);
            }
        }

        push(&mut self.cpu_hist, self.sys.global_cpu_usage().round() as u64);
        // scale net to KB/s for the sparkline
        push(&mut self.net_hist, (self.net_down / 1000.0).round() as u64);
    }
}

fn push(buf: &mut VecDeque<u64>, v: u64) {
    if buf.len() >= HIST {
        buf.pop_front();
    }
    buf.push_back(v);
}

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

fn fmt_rate(bps: f64) -> String {
    if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0} KB/s", bps / 1_000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

// ---------- entry ----------

const HELP: &str = "\
glowfetch — a live system-info TUI for Windows

USAGE:
    glowfetch [OPTIONS]

OPTIONS:
    -o, --once          Print a static snapshot and exit (neofetch style)
    -t, --theme <NAME>  windows | matrix | dracula | nord | amber
        --config <PATH> Use a specific config file
        --gen-config    Write a sample config to %APPDATA%\\glowfetch and exit
        --fancy         Force fancy glyphs on
        --no-fancy      Force fancy glyphs off
        --no-logo       Hide the ASCII logo
    -h, --help          Show this help
    -V, --version       Show version

Config is read from %APPDATA%\\glowfetch\\glowfetch.toml if present.
With no options, glowfetch opens the live dashboard (press q to quit).";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut once = false;
    let mut theme_override: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut fancy_override: Option<bool> = None;
    let mut logo_override: Option<bool> = None;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("glowfetch {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--gen-config" => {
                let path = default_config_path();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, SAMPLE_CONFIG)?;
                println!("wrote sample config to {}", path.display());
                return Ok(());
            }
            "-o" | "--once" => once = true,
            "--no-fancy" => fancy_override = Some(false),
            "--fancy" => fancy_override = Some(true),
            "--no-logo" => logo_override = Some(false),
            "-t" | "--theme" => {
                i += 1;
                theme_override = args.get(i).cloned();
            }
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            other => {
                eprintln!("glowfetch: unknown option '{other}' (try --help)");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // Load config (explicit path, else default location), then resolve settings.
    let path = config_path.map(std::path::PathBuf::from).unwrap_or_else(default_config_path);
    let cfg: Config = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    let _ = SETTINGS.set(build_settings(cfg, theme_override, fancy_override, logo_override));

    if once {
        print_once();
        return Ok(());
    }

    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    let mut last = Instant::now();

    loop {
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }

        let dt = last.elapsed().as_secs_f64();
        if dt >= 0.6 {
            app.tick(dt);
            last = Instant::now();
        }
    }
    Ok(())
}

// ---------- live UI ----------

fn draw(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    draw_header(f, root[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(42), Constraint::Min(38)])
        .split(root[1]);

    draw_card(f, body[0], app);
    draw_meters(f, body[1], app);
    draw_footer(f, root[2]);
}

fn draw_header(f: &mut Frame, area: Rect) {
    let left = Line::from(vec![
        Span::styled("  glowfetch", Style::default().fg(accent()).add_modifier(Modifier::BOLD)),
        Span::styled("  live system monitor", Style::default().fg(dim())),
    ]);
    f.render_widget(Paragraph::new(left).style(Style::default().bg(Color::Rgb(22, 24, 31))), area);
    let right = Paragraph::new(Line::from(Span::styled(
        format!("up {} ", fmt_uptime(System::uptime())),
        Style::default().fg(accent2()),
    )))
    .alignment(Alignment::Right)
    .style(Style::default().bg(Color::Rgb(22, 24, 31)));
    f.render_widget(right, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let key = |k: &str, d: &str| {
        vec![
            Span::styled(format!(" {k} "), Style::default().fg(Color::Black).bg(accent()).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {d}   "), Style::default().fg(dim())),
        ]
    };
    let mut spans = vec![];
    spans.extend(key("q", "quit"));
    spans.extend(key("esc", "quit"));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_card(f: &mut Frame, full: Rect, app: &App) {
    let logo_h = if st().show_logo { LOGO.len() as u16 + 1 } else { 0 };
    // Cap the card to its content height; leave the rest empty.
    let want = logo_h + 6 + if app.battery.is_some() { 1 } else { 0 } + 2;
    let area = Rect { height: want.min(full.height), ..full };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent()))
        .title(Span::styled(" system ", Style::default().fg(accent2()).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
    let cores = app.sys.cpus().len();
    let kv = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("{k:>8} ", ), Style::default().fg(accent()).add_modifier(Modifier::BOLD)),
            Span::styled(v, Style::default().fg(white())),
        ])
    };

    let (la, lb) = (st().theme.logo_a, st().theme.logo_b);
    let mut lines: Vec<Line> = Vec::new();
    if st().show_logo {
        for (i, l) in LOGO.iter().enumerate() {
            let c = lerp(la, lb, i as f64 / LOGO.len() as f64);
            lines.push(Line::from(Span::styled(*l, Style::default().fg(c).add_modifier(Modifier::BOLD))));
        }
        lines.push(Line::from(""));
    }
    lines.push(Line::from(vec![
        Span::styled(format!("  {user}"), Style::default().fg(accent2()).add_modifier(Modifier::BOLD)),
        Span::styled("@", Style::default().fg(dim())),
        Span::styled(app.host.clone(), Style::default().fg(accent()).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(Span::styled(
        format!("  {}", "─".repeat((user.len() + app.host.len() + 1).min(28))),
        Style::default().fg(dim()),
    )));
    lines.push(kv("os", app.os.clone()));
    lines.push(kv("kernel", app.kernel.clone()));
    lines.push(kv("cpu", format!("{} ({}c)", app.cpu_brand, cores)));
    lines.push(kv("gpu", app.gpu.clone()));
    lines.push(kv("display", format!("{} ({})", app.resolution, app.arch)));
    if let Some(b) = app.battery {
        lines.push(kv("battery", format!("{b}%")));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_meters(f: &mut Frame, area: Rect, app: &App) {
    let s = &st().sections;
    // Build the section list dynamically so toggled-off panels vanish cleanly.
    let mut kinds: Vec<(&str, u16)> = Vec::new();
    if s.cpu {
        kinds.push(("cpu", 10));
    }
    if s.ram {
        kinds.push(("ram", 3));
    }
    if s.disk {
        kinds.push(("disk", 3));
    }
    if s.net {
        kinds.push(("net", 8));
    }
    if s.palette {
        kinds.push(("palette", 3));
    }
    let mut constraints: Vec<Constraint> = kinds.iter().map(|(_, h)| Constraint::Length(*h)).collect();
    constraints.push(Constraint::Min(0)); // spacer

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, (kind, _)) in kinds.iter().enumerate() {
        let r = rows[i];
        match *kind {
            "cpu" => draw_cpu(f, r, app),
            "ram" => {
                let total = app.sys.total_memory();
                let used = app.sys.used_memory();
                let ram = if total > 0 { used as f64 / total as f64 } else { 0.0 };
                f.render_widget(gauge(title_for("RAM", "▥"), ram, format!("{:.1} / {:.1} GiB", gib(used), gib(total))), r);
            }
            "disk" => {
                let disk = if app.disk_total > 0 { app.disk_used as f64 / app.disk_total as f64 } else { 0.0 };
                f.render_widget(
                    gauge(title_for("DISK", "▤"), disk, format!("{:.0} / {:.0} GiB", gib(app.disk_used), gib(app.disk_total))),
                    r,
                );
            }
            "net" => draw_net(f, r, app),
            "palette" => draw_palette(f, r),
            _ => {}
        }
    }
}

// Section title with an optional icon when fancy glyphs are enabled.
fn title_for(plain: &str, icon: &str) -> String {
    if st().fancy {
        format!(" {icon} {plain} ")
    } else {
        format!(" {plain} ")
    }
}

fn gauge(title: String, ratio: f64, label: String) -> Gauge<'static> {
    Gauge::default()
        .block(titled(title))
        .gauge_style(Style::default().fg(load_color(ratio)).bg(track()))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label)
}

fn titled(title: String) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(dim()))
        .title(Span::styled(title, Style::default().fg(accent()).add_modifier(Modifier::BOLD)))
}

// A bar-graph built only from full blocks (█) — renders in any console font.
// Each column = one sample; height scaled to `max`. Columns colored per value.
fn render_graph(f: &mut Frame, area: Rect, hist: &VecDeque<u64>, max: u64, by_load: bool) {
    let h = area.height as usize;
    let w = area.width as usize;
    if h == 0 || w == 0 {
        return;
    }
    let max = max.max(1);
    // take the most recent `w` samples
    let data: Vec<u64> = hist.iter().rev().take(w).rev().copied().collect();
    let pad = w.saturating_sub(data.len());

    let mut lines: Vec<Line> = Vec::with_capacity(h);
    for row in 0..h {
        let level_from_bottom = h - row; // top row = h, bottom = 1
        let mut spans = Vec::with_capacity(w);
        for _ in 0..pad {
            spans.push(Span::raw(" "));
        }
        for &v in &data {
            let filled = ((v as f64 / max as f64) * h as f64).round() as usize;
            if filled >= level_from_bottom {
                let frac = v as f64 / max as f64;
                let color = if by_load {
                    load_color(frac)
                } else {
                    lerp(st().theme.accent, lerp_rgb(st().theme.accent, (255, 255, 255), 0.4), frac)
                };
                spans.push(Span::styled("█", Style::default().fg(color)));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_cpu(f: &mut Frame, area: Rect, app: &App) {
    let block = titled(title_for("CPU", "⚡"));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // per-core row: one solid block per core, colored by load
    let mut spans = vec![Span::styled("cores ", Style::default().fg(dim()))];
    for c in app.sys.cpus() {
        let u = c.cpu_usage() as f64 / 100.0;
        spans.push(Span::styled("█", Style::default().fg(load_color(u))));
    }
    let overall = app.sys.global_cpu_usage();
    spans.push(Span::styled(
        format!("   {overall:>3.0}%"),
        Style::default().fg(white()).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), parts[0]);

    render_graph(f, parts[1], &app.cpu_hist, 100, true);
}

fn draw_net(f: &mut Frame, area: Rect, app: &App) {
    let block = titled(title_for("NET", "⇅"));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let label = Line::from(vec![
        Span::styled("down ", Style::default().fg(Color::Rgb(40, 200, 120)).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:<12}", fmt_rate(app.net_down)), Style::default().fg(white())),
        Span::styled("up ", Style::default().fg(accent()).add_modifier(Modifier::BOLD)),
        Span::styled(fmt_rate(app.net_up), Style::default().fg(white())),
    ]);
    f.render_widget(Paragraph::new(label), parts[0]);

    let peak = app.net_hist.iter().copied().max().unwrap_or(1).max(10);
    render_graph(f, parts[1], &app.net_hist, peak, false);
}

fn draw_palette(f: &mut Frame, area: Rect) {
    let block = titled(title_for("palette", "◐"));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let (a, b) = (st().theme.accent, st().theme.accent2);
    let c2 = (240, 70, 140);
    let width = (inner.width as usize).max(1);
    let row = Line::from(
        (0..width)
            .map(|i| {
                let t = i as f64 / width as f64;
                let c = if t < 0.5 { lerp(a, b, t / 0.5) } else { lerp(b, c2, (t - 0.5) / 0.5) };
                Span::styled("█", Style::default().fg(c))
            })
            .collect::<Vec<_>>(),
    );
    f.render_widget(Paragraph::new(row), inner);
}

// ---------- static (--once) ----------

fn ansi(c: (u8, u8, u8), bold: bool, text: &str) -> String {
    let b = if bold { "\x1b[1m" } else { "" };
    format!("{b}\x1b[38;2;{};{};{}m{text}\x1b[0m", c.0, c.1, c.2)
}

fn print_once() {
    let _ = crossterm::execute!(io::stdout(), crossterm::style::ResetColor);
    let app = App::new();
    let a = st().theme.accent;
    let a2 = st().theme.accent2;
    let w = st().theme.text;
    let d = st().theme.dim;
    let cores = app.sys.cpus().len();
    let total = app.sys.total_memory();
    let used = app.sys.used_memory();

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
    let kv = |k: &str, v: String| format!("{}{}", ansi(a, true, &format!("{k}: ")), ansi(w, false, &v));

    let mut info = vec![
        format!("{}{}{}", ansi(a2, true, &user), ansi(d, false, "@"), ansi(a, true, &app.host)),
        ansi(d, false, &"─".repeat(user.len() + app.host.len() + 1)),
        kv("OS", app.os.clone()),
        kv("Kernel", app.kernel.clone()),
        kv("CPU", format!("{} ({}c)", app.cpu_brand, cores)),
        kv("GPU", app.gpu.clone()),
        kv("Display", format!("{} ({})", app.resolution, app.arch)),
        kv("Uptime", fmt_uptime(System::uptime())),
        kv("Memory", format!("{:.1} / {:.1} GiB", gib(used), gib(total))),
        kv("Disk", format!("{:.0} / {:.0} GiB", gib(app.disk_used), gib(app.disk_total))),
        kv("Terminal", terminal_name()),
    ];
    if let Some(b) = app.battery {
        info.push(kv("Battery", format!("{b}%")));
    }
    info.push(String::new());
    let blocks: String = [
        (0u8, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16),
        (36, 114, 200), (188, 63, 188), (17, 168, 205), (200, 200, 200),
    ]
    .iter()
    .map(|c| ansi(*c, false, "███"))
    .collect();
    info.push(blocks);

    println!();
    if st().show_logo {
        let rows = LOGO.len().max(info.len());
        for i in 0..rows {
            let logo_line = LOGO.get(i).copied().unwrap_or("");
            let lc = lerp_rgb(st().theme.logo_a, st().theme.logo_b, i as f64 / LOGO.len().max(1) as f64);
            let left = ansi(lc, true, &format!("{logo_line:<27}"));
            let right = info.get(i).cloned().unwrap_or_default();
            println!("{left}  {right}");
        }
    } else {
        for line in &info {
            println!("  {line}");
        }
    }
    println!();
}

fn terminal_name() -> String {
    if std::env::var("WT_SESSION").is_ok() {
        "Windows Terminal".into()
    } else if let Ok(tp) = std::env::var("TERM_PROGRAM") {
        tp
    } else if std::env::var("ConEmuPID").is_ok() {
        "ConEmu".into()
    } else {
        "conhost".into()
    }
}
