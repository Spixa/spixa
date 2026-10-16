use std::{cell::RefCell, io, rc::Rc};

use ratzilla::{
    event::KeyCode,
    ratatui::{
        layout::{Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph, Wrap},
        Terminal,
    },
    DomBackend, WebRenderer,
};

struct App {
    input: String,
    output: Vec<(String, bool)>,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            output: vec![
                ("CRT-OS v0.1.0".into(), false),
                ("hey, how're you doing?".into(), true),
                ("type 'help' to see available commands.".into(), true),
                (String::new(), false),
            ],
        }
    }

    fn execute(&mut self) {
        let command = self.input.trim().to_string();

        // Command entered by the user stays green.
        self.output.push((format!("guest@crt:~$ {command}"), false));

        // Command results are white.
        let mut response = Vec::new();

        match command.as_str() {
            "help" => {
                response.push("available commands: help, about, clear, date, whoami".to_string());
            }
            "about" => {
                response.push("Hi! This is a WASM test".to_string());
                response.push("Powered by Ratatui!".to_string());
            }
            "whoami" => {
                response.push("guest".to_string());
            }
            "date" => {
                response.push("Oh i dont know those stuff lol".to_string());
            }
            "clear" => {
                self.output.clear();
            }
            "" => {}
            _ => {
                response.push(format!("command not found: {command}"));
            }
        }

        self.output
            .extend(response.into_iter().map(|line| (line, true)));

        self.output.push((String::new(), false));
        self.input.clear();

        // Keep the UI from growing indefinitely.
        if self.output.len() > 200 {
            self.output.drain(..self.output.len() - 200);
        }
    }
}

fn main() -> io::Result<()> {
    let app = Rc::new(RefCell::new(App::new()));

    let backend = DomBackend::new_by_id("terminal")?;
    let mut terminal = Terminal::new(backend)?;

    terminal.on_key_event({
        let app = Rc::clone(&app);

        move |event| {
            let mut app = app.borrow_mut();

            match event.code {
                KeyCode::Enter => app.execute(),
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                _ => {}
            }
        }
    })?;

    terminal.draw_web(move |frame| {
        let app = app.borrow();

        let area = frame.area();

        // Narrower margins on small screens.
        let margin = if area.width < 60 { 0 } else { 2 };

        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(margin),
                Constraint::Min(0),
                Constraint::Length(margin),
            ])
            .split(area);

        let compact = area.width < 60 || area.height < 20;

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(if compact { 3 } else { 3 }),
            ])
            .split(outer[1]);

        let lines: Vec<Line<'_>> = app
            .output
            .iter()
            .map(|(text, is_response)| {
                let color = if *is_response {
                    Color::White
                } else {
                    Color::Green
                };

                Line::from(Span::styled(text.as_str(), Style::default().fg(color)))
            })
            .collect();

        let output = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" /BIN/TERMINAL -u guest")
                    .borders(Borders::ALL)
                    .border_style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(output, areas[0]);

        let prompt = Paragraph::new(Line::from(vec![
            Span::styled(
                "guest@crt:~$ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}_", app.input),
                Style::default().fg(Color::LightYellow),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );

        frame.render_widget(prompt, areas[1]);
    });

    Ok(())
}
