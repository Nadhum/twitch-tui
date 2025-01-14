use std::{
    io::{stdout, Stdout},
    time::Duration,
};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, info};
use tokio::sync::mpsc::{Receiver, Sender};
use tui::{backend::CrosstermBackend, Terminal};

use crate::{
    handlers::{
        app::{App, State},
        config::CompleteConfig,
        data::{Data, DataBuilder, PayLoad},
        user_input::{
            events::{Config, Events, Key},
            input::{handle_stateful_user_input, TerminalAction},
        },
    },
    twitch::TwitchAction,
    ui::draw_ui,
};

fn reset_terminal() {
    disable_raw_mode().unwrap();

    execute!(stdout(), LeaveAlternateScreen).unwrap();
}

fn init_terminal() -> Terminal<CrosstermBackend<Stdout>> {
    enable_raw_mode().unwrap();

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();

    let backend = CrosstermBackend::new(stdout);

    Terminal::new(backend).unwrap()
}

fn quit_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) {
    disable_raw_mode().unwrap();

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();

    terminal.show_cursor().unwrap();
}

pub async fn ui_driver(
    config: CompleteConfig,
    mut app: App,
    tx: Sender<TwitchAction>,
    mut rx: Receiver<Data>,
) {
    info!("Started UI driver.");

    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic| {
        debug!("Panic hook hit.");

        reset_terminal();
        original_hook(panic);
    }));

    let mut events = Events::with_config(Config {
        exit_key: Key::Null,
        tick_rate: Duration::from_millis(config.terminal.tick_delay),
    })
    .await;

    let mut terminal = init_terminal();

    terminal.clear().unwrap();

    let data_builder = DataBuilder::new(&config.frontend.date_format);

    loop {
        if let Ok(info) = rx.try_recv() {
            match info.payload {
                PayLoad::Message(_) => app.messages.push_front(info),

                // If something such as a keypress failed, fallback to the normal state of the application.
                PayLoad::Err(err) => {
                    app.state = State::Normal;

                    app.messages.push_front(data_builder.system(err));
                }
            }

            // If scrolling is enabled, pad for more messages.
            if app.scroll_offset > 0 {
                app.scroll_offset += 1;
            }
        }

        terminal
            .draw(|frame| draw_ui(frame, &mut app, &config))
            .unwrap();

        if let Some(TerminalAction::Quitting) =
            handle_stateful_user_input(&mut events, &mut app, &mut config.clone(), tx.clone()).await
        {
            quit_terminal(terminal);

            break;
        }
    }

    app.cleanup();

    reset_terminal();
}
