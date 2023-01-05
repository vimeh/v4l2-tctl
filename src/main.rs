use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Gauge, Tabs},
    Frame, Terminal,
};

use glob::glob;
use v4l::control::Value as ControlValue;
use v4l::prelude::*;

#[derive(Debug)]
struct Camera {
    name: String,
    progress: Vec<v4l::control::Description>,
    selected: usize,
}

impl Camera {
    fn new(name: &str) -> Camera {
        let mut ctls = Vec::new();
        if let Ok(dev) = Device::with_path(name) {
            if let Ok(mut controls) = dev.query_controls() {
                controls.retain_mut(|c| c.typ != v4l::control::Type::CtrlClass);
                for i in controls.iter_mut() {
                    if let Ok(c) = dev.control(i.id) {
                        i.default = match c.value {
                            ControlValue::Integer(n) => n,
                            _ => i.default,
                        }
                    }
                }
                ctls = controls;
            }
        }
        println!("{:?}", ctls);

        let c = Camera {
            name: String::from(name),
            progress: ctls,
            selected: 0,
        };
        return c;
    }
}

#[derive(Debug, Default)]
struct App {
    cams: Vec<Camera>,
    selected: usize,
}

impl App {
    fn new() -> App {
        let mut cams = Vec::new();
        for entry in glob("/dev/video*").expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    let c = Camera::new(&path.display().to_string());
                    cams.push(c);
                }
                Err(e) => println!("{:?}", e),
            }
        }

        App { cams, selected: 0 }
    }

    fn update(&mut self) {
        for cam in self.cams.iter_mut() {
            if let Ok(dev) = Device::with_path(&cam.name) {
                for mut prog in &mut cam.progress.iter_mut() {
                    if let Ok(ctl) = dev.control(prog.id) {
                        prog.default = match ctl.value {
                            ControlValue::Integer(n) => n,
                            _ => prog.default,
                        }
                    } else {
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // TODO: remove this later.
    #[cfg(debug_assertions)]
    {
        let path = "/dev/video0";
        println!("Using device: {}\n", path);

        let dev = Device::with_path(path)?;
        let controls = dev.query_controls()?;
        //println!("!!!!{:?}", controls);
        for control in controls {
            if let Ok(c) = dev.control(control.id) {
                println!("!!!!{:?}", c);
            }
            if control.typ == v4l::control::Type::Menu {
                if let Some(items) = &control.items {
                    for (k, v) in items.iter() {
                        println!("{} {}", k, v);
                    }
                }
            }
            //println!("{}", control);
        }
    }
    // setup terminimumal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminimumal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(30);
    let app = App::new();
    let res = run_app(&mut terminimumal, app, tick_rate);

    // restore terminimumal
    disable_raw_mode()?;
    execute!(
        terminimumal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminimumal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminimumal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminimumal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
                if let KeyCode::Tab = key.code {
                    if app.selected < app.cams.len() - 1 {
                        app.selected += 1;
                    }
                }
                if let KeyCode::BackTab = key.code {
                    if app.selected > 0 {
                        app.selected -= 1;
                    }
                }

                if let KeyCode::Char('j') | KeyCode::Down = key.code {
                    let mut cam = &mut app.cams[app.selected];
                    if cam.selected < cam.progress.len() - 1 {
                        cam.selected += 1;
                    }
                }
                if let KeyCode::Char('k') | KeyCode::Up = key.code {
                    let mut cam = &mut app.cams[app.selected];
                    if cam.selected > 0 {
                        cam.selected -= 1;
                    }
                }

                if let KeyCode::Char('l') | KeyCode::Right = key.code {
                    let cam = &mut app.cams[app.selected];
                    let mut i = &mut cam.progress[cam.selected];
                    let val = {
                        if i.default + (i.step as i64) > i.maximum {
                            i.maximum
                        } else {
                            i.default + i.step as i64
                        }
                    };
                    if let Ok(dev) = Device::with_path(&cam.name) {
                        let ctl = v4l::control::Control {
                            id: i.id,
                            value: v4l::control::Value::Integer(val),
                        };
                        if let Ok(_) = dev.set_control(ctl) {
                            i.default = val;
                        } else {
                        }
                    }
                }
                if let KeyCode::Char('h') | KeyCode::Left = key.code {
                    let cam = &mut app.cams[app.selected];
                    let mut i = &mut cam.progress[cam.selected];
                    let val = {
                        if i.default - (i.step as i64) > i.minimum {
                            i.default - i.step as i64
                        } else {
                            i.minimum
                        }
                    };
                    if let Ok(dev) = Device::with_path(&cam.name) {
                        let ctl = v4l::control::Control {
                            id: i.id,
                            value: v4l::control::Value::Integer(val),
                        };
                        if let Ok(_) = dev.set_control(ctl) {
                            i.default = val;
                        } else {
                        }
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.update();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let size = f.size();
    let block = Block::default().style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(block, size);
    let mut constraints = vec![Constraint::Length(3)];
    constraints.extend(vec![
        Constraint::Max(3);
        app.cams[app.selected].progress.len() + 1 //WTF?
    ]);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(constraints.as_ref())
        .split(f.size());
    let titles = app
        .cams
        .iter()
        .map(|t| {
            let (first, rest) = t.name.split_at(1);
            Spans::from(vec![
                Span::styled(first, Style::default().fg(Color::Yellow)),
                Span::styled(rest, Style::default().fg(Color::Green)),
            ])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.selected)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED)
                .bg(Color::Black),
        );
    f.render_widget(tabs, chunks[0]);

    for (i, p) in app.cams[app.selected].progress.iter().enumerate() {
        let color = if app.cams[app.selected].selected == i {
            [Color::White, Color::DarkGray]
        } else {
            [Color::DarkGray, Color::Black]
        };

        let ratio = (p.default - p.minimum) as f64 / (p.maximum - p.minimum) as f64;
        //let label = format!("{},{},{} {}", p.default, p.maximum, p.minimum, ratio);
        let mut label = format!("{}", p.default);

        match p.typ {
            v4l::control::Type::Menu => {
                if let Some(items) = &p.items {
                    for (k, v) in items.iter() {
                        if p.default == *k as i64 {
                            label = v.to_string();
                        }
                    }
                }
            }
            v4l::control::Type::Boolean => {
                label = if p.default == 1 {
                    "True".to_string()
                } else {
                    "False".to_string()
                };
            }
            _ => (),
        }

        let gauge = Gauge::default()
            .block(Block::default().title(&*p.name).borders(Borders::ALL))
            .gauge_style(Style::default().fg(color[0]).bg(color[1]))
            .ratio(ratio)
            .label(label);
        f.render_widget(gauge, chunks[i + 1]);
    }
}
