use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Terminal,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::io;

struct App {
    themes: Vec<String>,
    state: ListState,
    themes_dir: PathBuf,
    home: String,
}

impl App {
    fn new(themes: Vec<String>, themes_dir: PathBuf, home: String) -> Self {
        let mut state = ListState::default();
        if !themes.is_empty() {
            state.select(Some(0));
        }
        Self { themes, state, themes_dir, home }
    }

    fn next(&mut self) {
        if self.themes.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.themes.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.themes.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.themes.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn apply_selected(&self) {
        if let Some(i) = self.state.selected() {
            if let Some(theme) = self.themes.get(i) {
                apply_theme(theme, &self.themes_dir, &self.home);
            }
        }
    }
}

fn main() -> Result<(), io::Error> {
    // 1. Setup ambiente e recupero cartella Home
    let home = env::var("HOME").expect("Variabile HOME non trovata");
    let themes_dir = PathBuf::from(&home).join("dotfiles/themes");

    let mut themes = Vec::new();
    if let Ok(entries) = fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    themes.push(name.to_string());
                }
            }
        }
    }
    themes.sort();

    if themes.is_empty() {
        eprintln!("Nessun tema trovato in {:?}", themes_dir);
        return Ok(());
    }

    // 2. Setup del terminale in modalità raw per la TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(themes, themes_dir, home);
    let res = run_app(&mut terminal, app);

    // 3. Ripristino del terminale alla chiusura
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Errore nell'applicazione: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let size = f.area();
            
            // Layout principale: dividiamo lo schermo in blocchi
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(size);

            // Titolo in alto
            let title_block = Block::default()
                .borders(Borders::ALL)
                .title(" CachyOS Dotfiles - Theme Manager ");
            let title_widget = ratatui::widgets::Paragraph::new(" Seleziona un tema con le frecce (Su/Giù) e premi Invio ")
                .block(title_block)
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(title_widget, chunks[0]);

            // Lista dei temi al centro
            let items: Vec<ListItem> = app
                .themes
                .iter()
                .map(|t| ListItem::new(t.as_str()))
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Temi Disponibili "))
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, chunks[1], &mut app.state);

            // Barra dei comandi in basso
            let footer_block = Block::default().borders(Borders::ALL);
            let footer_widget = ratatui::widgets::Paragraph::new(" [Invio] Applica tema   [q / Esc] Esci ")
                .block(footer_block)
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(footer_widget, chunks[2]);
        })?;

        // Gestione degli input da tastiera
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Enter => {
                    app.apply_selected();
                    return Ok(()); // Esce dopo aver applicato il tema
                }
                _ => {}
            }
        }
    }
}

fn apply_theme(theme: &str, themes_dir: &Path, home: &str) {
    let theme_path = themes_dir.join(theme);

    // 1. Applica il tema di Kitty sovrascrivendo contemporaneamente tutti i file possibili
    let kitty_src = theme_path.join("kitty.conf");
    let kitty_dir = format!("{}/.config/kitty", home);
    
    if kitty_src.exists() {
        if let Ok(contents) = fs::read_to_string(&kitty_src) {
            let targets = vec![
                format!("{}/kitty.conf", kitty_dir),
                format!("{}/dark-theme.auto.conf", kitty_dir),
                format!("{}/light-theme.auto.conf", kitty_dir),
                format!("{}/current-theme.conf", kitty_dir),
            ];
            
            for target in targets {
                let _ = fs::write(&target, &contents);
            }
        }
    }

    // 2. Copia Waybar style (CSS)
    let waybar_style_src = theme_path.join("style.css");
    let waybar_style_dest = format!("{}/.config/waybar/style.css", home);
    if waybar_style_src.exists() {
        let _ = fs::copy(&waybar_style_src, &waybar_style_dest);
    }

    // 3. Copia Waybar config (JSONC) se presente
    let waybar_conf_src = theme_path.join("config.jsonc");
    let waybar_conf_dest = format!("{}/.config/waybar/config.jsonc", home);
    if waybar_conf_src.exists() {
        let _ = fs::copy(&waybar_conf_src, &waybar_conf_dest);
    }

    // 4. Copia Niri config (KDL) se presente
    let niri_src = theme_path.join("config.kdl");
    let niri_dest = format!("{}/.config/niri/config.kdl", home);
    if niri_src.exists() {
        let _ = fs::copy(&niri_src, &niri_dest);
    }

    // 5. Copia Fuzzel config se presente
    let fuzzel_src = theme_path.join("fuzzel.ini");
    let fuzzel_dest_dir = format!("{}/.config/fuzzel", home);
    let fuzzel_dest = format!("{}/fuzzel.ini", fuzzel_dest_dir);
    if fuzzel_src.exists() {
        let _ = fs::create_dir_all(&fuzzel_dest_dir);
        let _ = fs::copy(&fuzzel_src, &fuzzel_dest);
    }

    // 6. Applica lo sfondo con swaybg usando il flag corretto (-i)
    let extensions = ["png", "jpg", "jpeg"];
    let mut wallpaper_applied = false;
    for ext in extensions {
        let wallpaper_src = theme_path.join(format!("wallpaper.{}", ext));
        if wallpaper_src.exists() {
            if let Some(wallpaper_path) = wallpaper_src.to_str() {
                println!("Trovato sfondo: {}", wallpaper_path);
                
                // Termina eventuali istanze precedenti di swaybg
                let _ = Command::new("pkill").arg("swaybg").status();
                
                // Avvia swaybg con -i e -m fill
                let status = Command::new("swaybg")
                    .args(["-m", "fill", "-i", wallpaper_path])
                    .spawn();

                match status {
                    Ok(_) => println!("Sfondo avviato con successo via swaybg."),
                    Err(e) => eprintln!("Errore nell'avvio di swaybg: {}", e),
                }

                wallpaper_applied = true;
                break;
            }
        }
    }

    if !wallpaper_applied {
        println!("Nessun wallpaper.png/jpg trovato nella cartella del tema.");
    }

    // 7. Ricarica Waybar e Niri
    let _ = Command::new("pkill").arg("waybar").status();
    let _ = Command::new("waybar").spawn();
    let _ = Command::new("niri").args(["msg", "action", "reload-config"]).status();

    println!("Tema '{}' applicato con successo!", theme);
}