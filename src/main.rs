use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{prelude::*, widgets::*};
use std::{
    error::Error,
    io::{self, Stdout},
    process::{Command, Stdio},
};

enum Dir {
    Up,
    Down,
    Same,
}

#[derive(Clone)]
struct Handler {
    name: String,
    cmd: Vec<String>,
}

struct State {
    input: String,
    handlers: Vec<Handler>,
    filtered: Vec<Handler>,
    list_state: ListState,
}

impl State {
    fn new() -> Self {
        Self {
            input: String::new(),
            handlers: Vec::new(),
            filtered: Vec::new(),
            list_state: ListState::default(),
        }
    }

    fn load(&mut self) {
        self.handlers = vec![
            Handler {
                name: String::from("shutdown"),
                cmd: vec![String::from("systemctl"), String::from("poweroff")],
            },
            Handler {
                name: String::from("reboot"),
                cmd: vec![String::from("systemctl"), String::from("reboot")],
            },
            Handler {
                name: String::from("logout"),
                cmd: vec![
                    String::from("hyprctl"),
                    String::from("dispatch"),
                    String::from("exit"),
                ],
            },
        ];

        self.filter();
    }

    fn enter_char(&mut self, new_char: char) {
        self.input.insert(self.input.len(), new_char);
        self.filter();
    }

    fn delete_char(&mut self) {
        if !self.input.is_empty() {
            self.input = self.input.chars().take(self.input.len() - 1).collect();
            self.filter();
        }
    }

    fn filter(&mut self) {
        let matcher = SkimMatcherV2::default();

        self.filtered = self
            .handlers
            .clone()
            .into_iter()
            .filter(|h| matcher.fuzzy_match(&h.name, &self.input).is_some())
            .collect();

        self.move_index(Dir::Same);
    }

    fn move_index(&mut self, dir: Dir) {
        let len = self.filtered.len();

        if len == 0 {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(match dir {
                Dir::Down => Some(match self.list_state.selected() {
                    Some(i) => {
                        if i >= self.filtered.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                }),
                Dir::Up => Some(match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.filtered.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                }),
                Dir::Same => Some(match self.list_state.selected() {
                    Some(i) => i.clamp(0, self.filtered.len() - 1),
                    None => 0,
                }),
            })
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, Box<dyn Error>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(terminal.show_cursor()?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = setup_terminal()?;
    let res = run(&mut terminal, State::new());
    restore_terminal(&mut terminal)?;

    if let Ok(Some(handler)) = res {
        Command::new(&handler.cmd[0])
            .args(&handler.cmd[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?
            .wait()?;
    }

    Ok(())
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut state: State,
) -> io::Result<Option<Handler>> {
    state.load();

    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        state.move_index(Dir::Down);
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        state.move_index(Dir::Up);
                    }
                    KeyCode::Down => {
                        state.move_index(Dir::Down);
                    }
                    KeyCode::Up => {
                        state.move_index(Dir::Up);
                    }
                    KeyCode::Char(to_insert) => {
                        state.enter_char(to_insert);
                    }
                    KeyCode::Backspace => {
                        state.delete_char();
                    }
                    KeyCode::Enter => {
                        if let Some(i) = state.list_state.selected() {
                            return Ok(Some(state.filtered[i].clone()));
                        }
                    }
                    KeyCode::Esc => {
                        return Ok(None);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(frame: &mut Frame, state: &mut State) {
    let vertical = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]);
    let [search_area, handlers_area] = vertical.areas(frame.size());

    let search = Paragraph::new(state.input.as_str())
        .style(Style::default())
        .block(Block::bordered().title(" Search "));

    frame.render_widget(search, search_area);

    frame.set_cursor(
        search_area.x + state.input.len() as u16 + 1,
        search_area.y + 1,
    );

    let handlers: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|h| ListItem::new(Line::from(Span::raw(&h.name))))
        .collect();

    let handlers = List::new(handlers)
        .block(Block::bordered().title(" Handlers "))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::LightBlue),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(handlers, handlers_area, &mut state.list_state);
}
