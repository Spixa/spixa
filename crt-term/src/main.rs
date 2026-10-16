
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
    web_sys::{self, wasm_bindgen},
    DomBackend, WebRenderer,
};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Event, HtmlInputElement, KeyboardEvent};

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

        self.output
            .push((format!("guest@crt:~$ {command}"), false));

        let mut response = Vec::new();

        match command.as_str() {
            "help" => {
                response.push(
                    "commands: help, about, clear, date, whoami".to_string(),
                );
            }
            "about" => {
                response.push("Hi!".to_string());
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

        // Keep the output buffer bounded.
        if self.output.len() > 200 {
            let excess = self.output.len() - 200;
            self.output.drain(..excess);
        }
    }
}

fn setup_mobile_input(app: Rc<RefCell<App>>) -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window()
        .expect("window unavailable")
        .document()
        .expect("document unavailable");

    let input: HtmlInputElement = document
        .get_element_by_id("mobile-terminal-input")
        .ok_or_else(|| {
            wasm_bindgen::JsValue::from_str(
                "mobile-terminal-input element not found",
            )
        })?
        .dyn_into()?;

    // Sync typed characters from the HTML input into the terminal state.
    let app_for_input = Rc::clone(&app);
    let input_handler = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(target) = event.target() else {
            return;
        };

        let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
            return;
        };

        app_for_input.borrow_mut().input = input.value();
    });

    input.add_event_listener_with_callback(
        "input",
        input_handler.as_ref().unchecked_ref(),
    )?;
    input_handler.forget();

    // Execute commands when Enter is pressed on the mobile keyboard.
    let app_for_enter = Rc::clone(&app);
    let input_for_enter = input.clone();

    let enter_handler =
        Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if event.key() != "Enter" {
                return;
            }

            event.prevent_default();

            app_for_enter.borrow_mut().execute();
            input_for_enter.set_value("");
        });

    input.add_event_listener_with_callback(
        "keydown",
        enter_handler.as_ref().unchecked_ref(),
    )?;
    enter_handler.forget();

    Ok(())
}

fn main() -> io::Result<()> {
    let app = Rc::new(RefCell::new(App::new()));

    setup_mobile_input(Rc::clone(&app))
        .map_err(|e| io::Error::other(format!("{e:?}")))?;

    let backend = DomBackend::new_by_id("terminal")?;
    let mut terminal = Terminal::new(backend)?;

    // Desktop keyboard input.
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

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        let lines: Vec<Line<'_>> = app
            .output
            .iter()
            .map(|(text, is_response)| {
                let color = if *is_response {
                    Color::White
                } else {
                    Color::LightGreen
                };

                Line::from(Span::styled(
                    text.as_str(),
                    Style::default().fg(color),
                ))
            })
            .collect();

        let output = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" CRT-OS ")
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
        ]));

        frame.render_widget(prompt, areas[1]);
    });

    Ok(())
}
