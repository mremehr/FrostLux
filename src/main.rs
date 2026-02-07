mod app;
mod coap;
mod tradfri;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

use app::{load_config, App, Scene};
use ui::frost_theme_from_config;

fn main() -> Result<()> {
    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();

    // Check for --scene / -s flag (headless mode)
    if let Some(scene_arg) = parse_scene_arg(&args) {
        return run_headless_scene(&scene_arg);
    }

    // Check for --help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    // Initialize logging to file
    init_logging();

    // Load config
    let config = load_config().context("Failed to load config")?;

    // Validate credentials
    if config.gateway.identity.is_empty() || config.gateway.psk.is_empty() {
        eprintln!("Error: Gateway credentials not configured.");
        eprintln!("Edit ~/.config/frostlux/config.toml with your identity and psk.");
        eprintln!("\nTo pair with your gateway, use the security code on the back of it.");
        std::process::exit(1);
    }

    // Create app (connects to gateway via DTLS)
    let mut app = App::new(config).context("Failed to initialize FrostLux")?;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, SetTitle("FrostLux"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    // Guaranteed cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let refresh_interval = Duration::from_secs(app.config.ui.refresh_interval);
    let mut theme = frost_theme_from_config(&app.config.ui.theme);
    let mut last_theme_check = Instant::now();
    let theme_auto = app.config.ui.theme.eq_ignore_ascii_case("auto");

    // Initial fetch (blocking but necessary)
    app.set_status("Connecting to gateway...");
    if let Err(e) = app.refresh_lights() {
        app.set_status(&format!("Connection failed: {}", e));
    }

    loop {
        // Poll for completed background refresh
        app.poll_refresh();

        // Start background refresh when interval has elapsed
        if app.last_refresh.elapsed() >= refresh_interval {
            app.start_background_refresh();
        }

        // Auto theme detection refresh
        if theme_auto && last_theme_check.elapsed() >= Duration::from_secs(2) {
            theme = frost_theme_from_config(&app.config.ui.theme);
            last_theme_check = Instant::now();
        }

        // Draw
        terminal.draw(|f| ui::draw(f, app, &theme))?;

        // Handle input
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Help popup blocks other input
                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter => {
                            app.show_help = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    // Quit
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,

                    // Navigation
                    KeyCode::Char('j') | KeyCode::Down => app.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => app.select_prev(),

                    // Toggle
                    KeyCode::Char(' ') => {
                        if let Err(e) = app.toggle_selected() {
                            app.set_status(&format!("Error: {}", e));
                        }
                    }

                    // Brightness
                    KeyCode::Char('h') | KeyCode::Left => {
                        let _ = app.dim_selected(-25);
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        let _ = app.dim_selected(25);
                    }
                    KeyCode::PageDown => {
                        let _ = app.dim_selected(-64);
                    }
                    KeyCode::PageUp => {
                        let _ = app.dim_selected(64);
                    }

                    // Color temperature
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        let _ = app.cycle_color_temp(true);
                    }
                    KeyCode::Char('-') => {
                        let _ = app.cycle_color_temp(false);
                    }

                    // Scenes
                    KeyCode::Char('a') => {
                        let _ = app.apply_scene(Scene::AllOn);
                    }
                    KeyCode::Char('o') => {
                        let _ = app.apply_scene(Scene::AllOff);
                    }
                    KeyCode::Char('m') => {
                        let _ = app.apply_scene(Scene::Movie);
                    }
                    KeyCode::Char('b') => {
                        let _ = app.apply_scene(Scene::Bright);
                    }
                    KeyCode::Char('c') => {
                        let _ = app.apply_scene(Scene::Cozy);
                    }
                    KeyCode::Char('n') => {
                        let _ = app.apply_scene(Scene::Night);
                    }
                    KeyCode::Char('e') => {
                        let _ = app.apply_scene(Scene::Evening);
                    }
                    KeyCode::Char('r') => {
                        let _ = app.apply_scene(Scene::Reading);
                    }
                    KeyCode::Char('g') => {
                        let _ = app.apply_scene(Scene::GoodMorning);
                    }

                    // Force refresh
                    KeyCode::Char('R') => {
                        if let Err(e) = app.refresh_lights() {
                            app.set_status(&format!("Refresh failed: {}", e));
                        } else {
                            app.set_status("Refreshed");
                        }
                    }

                    // Help
                    KeyCode::Char('?') => app.show_help = true,

                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn parse_scene_arg(args: &[String]) -> Option<String> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--scene" || arg == "-s" {
            return iter.next().cloned();
        }
        if let Some(stripped) = arg.strip_prefix("--scene=") {
            return Some(stripped.to_string());
        }
    }
    None
}

fn run_headless_scene(scene_name: &str) -> Result<()> {
    let config = load_config().context("Failed to load config")?;

    if config.gateway.identity.is_empty() || config.gateway.psk.is_empty() {
        anyhow::bail!("Gateway credentials not configured in ~/.config/frostlux/config.toml");
    }

    let scene = Scene::from_str(scene_name).with_context(|| {
        format!(
            "Unknown scene: '{}'\n\nAvailable scenes: {}",
            scene_name,
            Scene::all()
                .iter()
                .map(|s| s.name().to_lowercase().replace(' ', "-"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    App::run_scene_headless(&config, scene)
}

fn print_help() {
    println!(
        r#"FrostLux — TUI controller for IKEA Tradfri smart lights

USAGE:
    frostlux              Launch interactive TUI
    frostlux --scene NAME Apply a scene directly (no TUI)
    frostlux --help       Show this help

SCENES:
    on, off, movie, bright, cozy, night, evening, reading, morning

EXAMPLES:
    frostlux --scene movie     Apply movie scene
    frostlux -s off            Turn all lights off
    frostlux -s cozy           Apply cozy scene

CONFIG:
    ~/.config/frostlux/config.toml

    [gateway]
    host = "192.168.0.131"
    identity = "tradfri_xxx"
    psk = "your_psk"

    [scenes]
    exclude = ["Sovrummet"]    # Skip in all scenes
    exclude_by_scene = {{ movie = ["TV-lampan"], night = ["Kök"] }}
"#
    );
}

fn init_logging() {
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("frostlux");
    let _ = std::fs::create_dir_all(&log_dir);

    if let Ok(file) = std::fs::File::create(log_dir.join("frostlux.log")) {
        let _ = tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();
    }
}
