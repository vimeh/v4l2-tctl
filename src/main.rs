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

//use glob::glob;
use v4l::control::Value as ControlValue;
use v4l::prelude::*;

#[derive(Debug)]
struct Camera {
    name: String,
    progress: Vec<v4l::control::Description>,
    selected: usize,
}

impl Camera {
    fn new() -> Camera {
        //let mut paths = Vec::new();
        //for entry in glob("/dev/video*").expect("Failed to read glob pattern") {
        //    match entry {
        //        Ok(path) => {
        //            paths.push(path.display().to_string());
        //        }
        //        Err(e) => println!("{:?}", e),
        //    }
        //}
        let mut ctls = Vec::new();
        if let Ok(dev) = Device::with_path("/dev/video0") {
            if let Ok(mut controls) = dev.query_controls() {
                //devices.push(&paths);
                controls.retain_mut(|c| c.typ == v4l::control::Type::Integer);
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
        //println!("{:?}", ctls);

        let c = Camera {
            name: String::from("/dev/video0"),
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
        App {
            cams: vec![Camera::new(), Camera::new(), Camera::new(), Camera::new()],
            selected: 0,
        }
    }

    fn update(&mut self) {
        for i in self.cams[0].progress.iter_mut() {
            if i.default + (i.step as i64) > i.maximum {
                i.default = i.maximum;
            } else if i.default + (i.step as i64) < i.minimum {
                i.default = i.minimum
            } else {
                i.default += i.step as i64
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = "/dev/video0";
    println!("Using device: {}\n", path);

    let dev = Device::with_path(path)?;
    let controls = dev.query_controls()?;

    for control in controls {
        if let Ok(c) = dev.control(control.id) {
            println!("!!!!{:?}", c);
        }
        //println!("{}", control);
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

                if let KeyCode::Char('j') = key.code {
                    let mut cam = &mut app.cams[app.selected];
                    if cam.selected < cam.progress.len() - 1 {
                        cam.selected += 1;
                    }
                }
                if let KeyCode::Char('k') = key.code {
                    let mut cam = &mut app.cams[app.selected];
                    if cam.selected > 0 {
                        cam.selected -= 1;
                    }
                }

                if let KeyCode::Char('l') = key.code {
                    let c = &mut app.cams[app.selected];
                    let mut i = &mut c.progress[c.selected];
                    if i.default + (i.step as i64) > i.maximum {
                        i.default = i.maximum;
                    } else {
                        i.default += i.step as i64
                    }
                    if let Ok(dev) = Device::with_path(&c.name) {
                        let xx = v4l::control::Control {
                            id: i.id,
                            value: v4l::control::Value::Integer(i.default),
                        };
                        if let Ok(_) = dev.set_control(xx) {} else {
                            i.default = -99
                        }
                    }
                }
                if let KeyCode::Char('h') = key.code {
                    let c = &mut app.cams[app.selected];
                    let mut i = &mut c.progress[c.selected];
                    if i.default - (i.step as i64) > i.minimum {
                        i.default -= i.step as i64;
                    } else {
                        i.default = i.minimum;
                    }
                    if let Ok(dev) = Device::with_path(&c.name) {
                        let xx = v4l::control::Control {
                            id: i.id,
                            value: v4l::control::Value::Integer(i.default),
                        };
                        if let Ok(_) = dev.set_control(xx) {} else {
                            i.default = -99
                        }
                    }

                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            //app.update();
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
        //Constraint::Ratio(
        //    1,
        //    app.cams[app.selected].progress.len() as u32
        //);
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
        //if p.typ != v4l::control::Type::Integer {
        //    continue;
        //}
        let label = format!("{}", p.default);
        let color = if app.cams[app.selected].selected == i {
            [Color::Green, Color::Black]
        } else {
            [Color::DarkGray, Color::Gray]
        };

        let gauge = Gauge::default()
            .block(Block::default().title(&*p.name).borders(Borders::ALL))
            .gauge_style(Style::default().fg(color[0]).bg(color[1]))
            .ratio((p.default / (p.maximum - p.minimum)) as f64)
            .label(label);
        f.render_widget(gauge, chunks[i + 1]);
    }
}
