use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::theme::FrostTheme;

// Compact layered snowflake: keeps the frosted look but fits tighter terminals.
const SNOWFLAKE_OUTER: [&str; 5] = [
    "  ·   ✶   ·  ",
    " ·         · ",
    "    ·   ·    ",
    " ·         · ",
    "  ·   ✶   ·  ",
];

const SNOWFLAKE_MID: [&str; 5] = [
    "      │      ",
    "   ╲  │  ╱   ",
    " ━━━     ━━━ ",
    "   ╱  │  ╲   ",
    "      │      ",
];

const SNOWFLAKE_CORE: [&str; 5] = [
    "      ✦      ",
    "     ❄❄❄     ",
    "    ❄❄❄❄❄    ",
    "     ❄❄❄     ",
    "      ✦      ",
];

pub fn draw(frame: &mut Frame, app: &App, theme: &FrostTheme) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Header with snowflake
            Constraint::Min(5),     // Light list
            Constraint::Length(3),  // Footer with controls
        ])
        .split(area);

    draw_header(frame, chunks[0], app, theme);
    draw_light_list(frame, chunks[1], app, theme);
    draw_footer(frame, chunks[2], app, theme);

    // Status message overlay
    if let Some(msg) = app.current_status() {
        draw_status_popup(frame, area, msg, theme);
    }

    // Help overlay
    if app.show_help {
        draw_help_popup(frame, area, theme);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App, theme: &FrostTheme) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(15), // Snowflake
            Constraint::Min(20),    // Title + stats
        ])
        .split(area);

    // Glowing snowflake - layered effect
    let snowflake_lines: Vec<Line> = (0..5)
        .map(|i| {
            let mut spans = Vec::new();
            let outer = SNOWFLAKE_OUTER[i];
            let mid = SNOWFLAKE_MID[i];
            let core = SNOWFLAKE_CORE[i];

            // Combine layers character by character
            for ((o, m), c) in outer.chars().zip(mid.chars()).zip(core.chars()) {
                let ch = if c != ' ' {
                    c  // Core layer (brightest)
                } else if m != ' ' {
                    m  // Mid layer
                } else {
                    o  // Outer glow
                };

                let style = if c != ' ' {
                    // Core: bright crystal center with warm sparkle accents
                    match c {
                        '✦' => Style::default().fg(theme.warm_yellow).add_modifier(Modifier::BOLD),
                        _ => Style::default().fg(theme.crystal_cyan).add_modifier(Modifier::BOLD),
                    }
                } else if m != ' ' {
                    // Mid: ice blue
                    Style::default().fg(theme.ice_blue).add_modifier(Modifier::BOLD)
                } else if o != ' ' {
                    // Outer glow: dimmed
                    if o == '✶' {
                        Style::default().fg(theme.warm_yellow)
                    } else {
                        Style::default().fg(theme.dimmed)
                    }
                } else {
                    Style::default()
                };

                spans.push(Span::styled(ch.to_string(), style));
            }
            Line::from(spans)
        })
        .collect();

    let snowflake = Paragraph::new(snowflake_lines);
    frame.render_widget(snowflake, header_chunks[0]);

    // Title and stats
    let on = app.lights_on();
    let off = app.lights_off();
    let total = app.lights.len();

    let title_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "FrostLux",
            Style::default()
                .fg(theme.ice_blue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(format!("{} ", on), Style::default().fg(theme.cold_green)),
            Span::styled("ON", Style::default().fg(theme.dimmed)),
            Span::styled("  ·  ", Style::default().fg(theme.dimmed)),
            Span::styled(format!("{} ", off), Style::default().fg(theme.bright_red)),
            Span::styled("OFF", Style::default().fg(theme.dimmed)),
            Span::styled("  ·  ", Style::default().fg(theme.dimmed)),
            Span::styled(format!("{} ", total), Style::default().fg(theme.foreground)),
            Span::styled("TOTAL", Style::default().fg(theme.dimmed)),
        ]),
    ];
    let title = Paragraph::new(title_lines);
    frame.render_widget(title, header_chunks[1]);
}

fn draw_light_list(frame: &mut Frame, area: Rect, app: &App, theme: &FrostTheme) {
    let items: Vec<ListItem> = app
        .lights
        .iter()
        .enumerate()
        .map(|(i, light)| {
            let is_selected = i == app.selected;

            // Status icon and label (reachable lights are controllable).
            let (icon, icon_color, state_label, state_color) = if !light.reachable {
                ("!", theme.bright_red, "UNR ", theme.bright_red)
            } else if light.on {
                ("*", theme.cold_green, " ON ", theme.cold_green)
            } else {
                (".", theme.dimmed, "OFF ", theme.bright_red)
            };

            // Name (max 25 chars)
            let name = if light.name.len() > 25 {
                format!("{}...", &light.name[..22])
            } else {
                format!("{:25}", light.name)
            };

            // Brightness bar (10 segments)
            let pct = light.brightness_percent() as usize;
            let filled = pct / 10;
            let bar: String = "█".repeat(filled) + &"░".repeat(10 - filled);

            // Color temp indicator
            let temp_label = light.color_temp_label();
            let temp_indicator = match temp_label {
                "warm" => "●",
                "cold" => "○",
                _ => " ",
            };
            let temp_color = if temp_label == "warm" {
                theme.warm_yellow
            } else {
                theme.crystal_cyan
            };

            let line = Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(icon_color)),
                Span::styled(name, if is_selected { theme.selected() } else { theme.normal() }),
                Span::raw("  "),
                Span::styled(state_label, Style::default().fg(state_color)),
                Span::styled(bar, Style::default().fg(theme.ice_blue)),
                Span::styled(format!(" {:>3}%", pct), Style::default().fg(theme.foreground)),
                Span::raw("  "),
                Span::styled(temp_indicator, Style::default().fg(temp_color)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border())
                .title(Span::styled(" Lights ", theme.title())),
        )
        .style(theme.normal());

    frame.render_widget(list, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, _app: &App, theme: &FrostTheme) {
    let sep = Span::styled("  ", Style::default());

    let line1 = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(theme.ice_blue)),
        Span::styled(" nav", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("Space", Style::default().fg(theme.ice_blue)),
        Span::styled(" toggle", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("h/l", Style::default().fg(theme.ice_blue)),
        Span::styled(" dim", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("+/-", Style::default().fg(theme.ice_blue)),
        Span::styled(" color", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("?", Style::default().fg(theme.ice_blue)),
        Span::styled(" help", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("q", Style::default().fg(theme.ice_blue)),
        Span::styled(" quit", Style::default().fg(theme.dimmed)),
    ]);

    let line2 = Line::from(vec![
        Span::styled(" a", Style::default().fg(theme.warm_yellow)),
        Span::styled(" on", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("o", Style::default().fg(theme.warm_yellow)),
        Span::styled(" off", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("m", Style::default().fg(theme.warm_yellow)),
        Span::styled(" movie", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("b", Style::default().fg(theme.warm_yellow)),
        Span::styled(" bright", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("c", Style::default().fg(theme.warm_yellow)),
        Span::styled(" cozy", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("n", Style::default().fg(theme.warm_yellow)),
        Span::styled(" night", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("e", Style::default().fg(theme.warm_yellow)),
        Span::styled(" evening", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("r", Style::default().fg(theme.warm_yellow)),
        Span::styled(" read", Style::default().fg(theme.dimmed)),
        sep.clone(),
        Span::styled("g", Style::default().fg(theme.warm_yellow)),
        Span::styled(" morning", Style::default().fg(theme.dimmed)),
    ]);

    let footer = Paragraph::new(vec![line1, line2])
        .block(Block::default().borders(Borders::TOP).border_style(theme.border()));

    frame.render_widget(footer, area);
}

fn draw_status_popup(frame: &mut Frame, area: Rect, msg: &str, theme: &FrostTheme) {
    let width = (msg.len() + 4).min(50) as u16;
    let height = 3;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(Line::from(Span::styled(
        msg,
        Style::default()
            .fg(theme.foreground)
            .add_modifier(Modifier::BOLD),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.cold_green)),
    )
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(popup, popup_area);
}

fn draw_help_popup(frame: &mut Frame, area: Rect, theme: &FrostTheme) {
    let width = 50;
    let height = 18;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled("Navigation", Style::default().fg(theme.ice_blue).add_modifier(Modifier::BOLD))),
        Line::from("  j / ↓      Next light"),
        Line::from("  k / ↑      Previous light"),
        Line::from(""),
        Line::from(Span::styled("Control", Style::default().fg(theme.ice_blue).add_modifier(Modifier::BOLD))),
        Line::from("  Space      Toggle on/off"),
        Line::from("  h / ←      Dim -10%"),
        Line::from("  l / →      Dim +10%"),
        Line::from("  PgUp/Dn    Dim ±25%"),
        Line::from("  + / -      Color temp warmer/colder"),
        Line::from(""),
        Line::from(Span::styled("Scenes", Style::default().fg(theme.warm_yellow).add_modifier(Modifier::BOLD))),
        Line::from("  a=On o=Off m=Movie b=Bright c=Cozy"),
        Line::from("  n=Night e=Evening r=Read g=Morning"),
        Line::from(""),
        Line::from(Span::styled("  Press ? or Esc to close", Style::default().fg(theme.dimmed))),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(Span::styled(" FrostLux Help ", theme.title()))
                .borders(Borders::ALL)
                .border_style(theme.border()),
        )
        .style(theme.normal());

    frame.render_widget(help, popup_area);
}
