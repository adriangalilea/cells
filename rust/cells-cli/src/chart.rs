//! `cells chart`: a file's charts in a terminal (SPEC.md, Charts). The engine says what each
//! chart draws (`Doc::summary`); here it is drawn into a terminal's cells in what the
//! terminal holds: octants or braille, half blocks and eighths, or plain ASCII; true
//! colour, 256 or 16. Once inline at the terminal's width, or held on the alternate screen
//! and drawn again as the file changes (`--watch`); `--svg` writes the engine's SVG.
//!
//! A terminal that answers the cell border protocol (joshka.net/cell-borders, the spec in
//! github.com/joshka/ghostty `specs/cell-border-protocol.md`) is given the card's frame, its
//! gridlines and its axis as lines on the cells' edges, so they cost no row or column; every
//! other terminal gets box drawing, byte for byte as before.

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cells::chart::{Kind, Shade, Stack, Summary};
use cells_disk::Disk;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui::widgets::canvas::{Canvas, Line as Segment, Points};

/// What a terminal draws with: its glyphs, its colours, and lines on its cells' edges when
/// it answers for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub glyphs: Glyphs,
    pub colors: Colors,
    pub edges: Option<Edges>,
}

/// What a terminal said of the cell border protocol: it draws lines on cells' edges (its
/// `pen` and `rect`), and whether it rounds their corners (`round`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edges {
    pub round: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphs {
    /// Octants (2x4 solid) where the font holds them, braille's dots otherwise.
    Fine,
    /// Half blocks and eighths: what a console font holds.
    Blocks,
    Ascii,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colors {
    True,
    Indexed,
    Sixteen,
}

impl Tier {
    /// What the terminal says it holds: `TERM=linux` is the kernel's console (sixteen
    /// colours, a console font of blocks and no braille), `COLORTERM` says true colour. No
    /// variable says cell borders (the protocol forbids guessing them), so none are guessed:
    /// the terminal is asked (`Edges::asked`).
    pub fn guessed() -> Tier {
        let term = std::env::var("TERM").unwrap_or_default();
        let truecolor = matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit"));
        if term == "linux" {
            return Tier { glyphs: Glyphs::Blocks, colors: Colors::Sixteen, edges: None };
        }
        if term == "dumb" {
            return Tier { glyphs: Glyphs::Ascii, colors: Colors::Sixteen, edges: None };
        }
        Tier {
            glyphs: Glyphs::Fine,
            colors: if truecolor {
                Colors::True
            } else if term.contains("256") {
                Colors::Indexed
            } else {
                Colors::Sixteen
            },
            edges: None,
        }
    }
}

/// The capability query, then Primary Device Attributes: terminals answer in order, so a
/// DA1 reply with no border reply before it is a terminal without the protocol.
const QUERY: &[u8] = b"\x1bP1+bcell-border;q\x1b\\\x1b[c";

impl Edges {
    /// The terminal asked, in raw mode for as long as its answers take: the query and DA1
    /// written to the terminal, its replies read from `/dev/tty` until DA1's ends them. A
    /// terminal that answers neither within half a second is taken as one without; the
    /// reader is left waiting for that late answer, so it never reaches the shell as typing.
    fn asked() -> Option<Edges> {
        use ratatui::crossterm::terminal;
        let mut tty = fs::OpenOptions::new().read(true).open("/dev/tty").ok()?;
        terminal::enable_raw_mode().ok()?;
        let (said, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut replies = Vec::new();
            let mut chunk = [0u8; 256];
            while !da1_ends(&replies) {
                match tty.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => replies.extend_from_slice(&chunk[..n]),
                }
            }
            let _ = said.send(replies);
        });
        let mut out = io::stdout().lock();
        let asked = out.write_all(QUERY).and_then(|_| out.flush());
        let replies = asked.ok().and_then(|_| heard.recv_timeout(Duration::from_millis(500)).ok());
        terminal::disable_raw_mode().ok()?;
        Edges::answered(&replies?)
    }

    /// What the replies to `QUERY` say: a border reply before DA1's, `DCS 1 + b
    /// cell-border;ok;VERSION;FEATURES ST`, naming `pen`, `rect` and `center` (the rules
    /// run through their rows' middles), is a terminal that draws them; `round` among its
    /// features, one that rounds corners.
    fn answered(replies: &[u8]) -> Option<Edges> {
        let da1 = find(replies, b"\x1b[?")?;
        let before = &replies[..da1];
        let reply = b"\x1bP1+bcell-border;ok;";
        let at = find(before, reply)? + reply.len();
        let rest = &before[at..];
        let end = find(rest, b"\x1b\\")?;
        let fields = std::str::from_utf8(&rest[..end]).ok()?;
        let (version, features) = fields.split_once(';')?;
        version.parse::<u32>().ok().filter(|v| *v >= 1)?;
        let has = |f: &str| features.split(',').any(|x| x == f);
        (has("pen") && has("rect") && has("center")).then_some(Edges { round: has("round") })
    }
}

/// Whether `bytes` holds a whole DA1 reply, `CSI ? Ps ; … c`.
fn da1_ends(bytes: &[u8]) -> bool {
    find(bytes, b"\x1b[?").is_some_and(|at| {
        let rest = &bytes[at + 3..];
        rest.iter().position(|b| !(b.is_ascii_digit() || *b == b';')).is_some_and(|i| rest[i] == b'c')
    })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// What `cells chart` was asked.
struct Asked {
    path: PathBuf,
    charts: Vec<usize>,
    watch: bool,
    svg: bool,
    tier: Tier,
    /// `--edges on|off`, said; none, the terminal is asked.
    edges: Option<bool>,
}

/// `cells chart <file> [n …] [--watch] [--svg] [--glyphs fine|blocks|ascii]
/// [--colors true|256|16] [--edges on|off]`. No numbers lists the file's charts.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut asked = Asked { path: PathBuf::new(), charts: Vec::new(), watch: false, svg: false, tier: Tier::guessed(), edges: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--watch" => asked.watch = true,
            "--svg" => asked.svg = true,
            "--edges" => {
                asked.edges = match it.next().map(String::as_str) {
                    Some("on") => Some(true),
                    Some("off") => Some(false),
                    other => return Err(format!("--edges on or off, not {other:?}")),
                }
            }
            "--glyphs" => {
                asked.tier.glyphs = match it.next().map(String::as_str) {
                    Some("fine") => Glyphs::Fine,
                    Some("blocks") => Glyphs::Blocks,
                    Some("ascii") => Glyphs::Ascii,
                    other => return Err(format!("--glyphs fine, blocks or ascii, not {other:?}")),
                }
            }
            "--colors" => {
                asked.tier.colors = match it.next().map(String::as_str) {
                    Some("true") => Colors::True,
                    Some("256") => Colors::Indexed,
                    Some("16") => Colors::Sixteen,
                    other => return Err(format!("--colors true, 256 or 16, not {other:?}")),
                }
            }
            n if n.parse::<usize>().is_ok() => asked.charts.push(n.parse().unwrap()),
            p if asked.path.as_os_str().is_empty() => asked.path = PathBuf::from(p),
            other => return Err(format!("chart: what is {other}?")),
        }
    }
    if asked.path.as_os_str().is_empty() {
        return Err("chart <file> [n …]: which file?".to_owned());
    }
    let summaries = |path: &Path| -> Result<(Vec<(String, Kind)>, Vec<Summary>), String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let doc = cells::read_with(bytes, &mut Disk::new(path), 1).map_err(|e| format!("{} {e}", path.display()))?;
        // chart 0 is the one a file with none is drawn from
        let listed: Vec<(String, Kind)> = match doc.charts() {
            [] => doc.inferred_chart().map(|c| (c.title.clone(), c.kind)).into_iter().collect(),
            charts => charts.iter().map(|c| (c.title.clone(), c.kind)).collect(),
        };
        let numbered = !doc.charts().is_empty();
        let mut drawn = Vec::new();
        for &n in &asked.charts {
            let s = match (numbered, n) {
                (true, n) if n >= 1 && n <= doc.charts().len() => doc.summary(&doc.charts()[n - 1]),
                (false, 0) => match doc.inferred_chart() {
                    Some(c) => doc.summary(&c),
                    None => return Err(format!("{}: nothing to chart", path.display())),
                },
                _ => return Err(format!("{}: no chart {n} (cells chart {} lists them)", path.display(), path.display())),
            };
            drawn.push(s);
        }
        Ok((listed.into_iter().map(|(t, k)| (t, k)).collect(), drawn))
    };
    let (listed, drawn) = summaries(&asked.path)?;
    if asked.charts.is_empty() {
        let first = if listed.len() == 1 && !has_charts(&asked.path) { 0 } else { 1 };
        if listed.is_empty() {
            println!("{}: nothing to chart (no date, choice or number)", asked.path.display());
        }
        for (i, (title, kind)) in listed.iter().enumerate() {
            println!("{:>3}  {}  {}", i + first, title, kind.word());
        }
        return Ok(());
    }
    if asked.svg {
        for s in &drawn {
            print!("{}", s.svg(720, 320));
        }
        return Ok(());
    }
    // `--edges on` is a terminal said to draw them, rounding too: a terminal without
    // `round` ignores the joint operations and draws the corners square
    asked.tier.edges = match asked.edges {
        Some(true) => Some(Edges { round: true }),
        Some(false) => None,
        None if io::stdout().is_terminal() => Edges::asked(),
        None => None,
    };
    if asked.watch {
        return watch(&asked, &summaries);
    }
    let width = terminal_width();
    // the plot's rows, and what stands round them: the margins, title, legend, the gap
    // and the places
    let height = 14 + 7;
    let mut out = io::stdout().lock();
    for s in &drawn {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let mut borders = Borders::new(area);
        let card = draw(
            s,
            area,
            &mut buf,
            &mut borders,
            asked.tier,
            Foot { said: vec![Span::raw(format!(" {} ", counted(s)))], keys: Vec::new() },
        );
        buf.resize(Rect::new(0, 0, width, card.height));
        print(&buf, &borders, asked.tier, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn has_charts(path: &Path) -> bool {
    fs::read(path).ok().and_then(|b| cells::read_with(b, &mut Disk::new(path), 1).ok()).is_some_and(|d| !d.charts().is_empty())
}

fn terminal_width() -> u16 {
    ratatui::crossterm::terminal::size().map(|(w, _)| w).ok().filter(|w| *w > 0).unwrap_or(80)
}

/// The charts held on the alternate screen, drawn again when the file changes or the
/// terminal is resized, the last card's foot saying the file, how long ago it was read and
/// the rows charted, and apart from them the keys. `q`, Esc or Ctrl+C leaves; a file that
/// does not read keeps the charts it last drew, why in the foot.
fn watch(asked: &Asked, summaries: &dyn Fn(&Path) -> Result<(Vec<(String, Kind)>, Vec<Summary>), String>) -> Result<(), String> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
    use std::time::Instant;
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut seen = modified(&asked.path);
    let mut drawn = summaries(&asked.path).map(|(_, d)| d)?;
    let mut refused: Option<String> = None;
    let mut read = Instant::now();
    let name = asked.path.file_name().map_or_else(|| asked.path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut terminal = ratatui::init();
    // the borders of the frame drawn, and of the one before: ratatui writes its cells with
    // no pen, so every cell it writes loses its borders and each frame's are set again
    let mut borders = Borders::new(Rect::default());
    let mut before: Option<Borders> = None;
    let result = loop {
        let frame = terminal.draw(|f| {
            let area = f.area();
            borders = Borders::new(area);
            let n = drawn.len().max(1) as u16;
            let h = area.height / n;
            let ago = read.elapsed().as_secs();
            for (i, s) in drawn.iter().enumerate() {
                // the last card's foot says what is read and the keys
                let foot = if i + 1 == drawn.len() {
                    let mut said = vec![Span::raw(format!(
                        " {name} · read {} · {}",
                        if ago < 2 { "now".to_owned() } else { format!("{ago}s ago") },
                        counted(s)
                    ))];
                    if let Some(why) = &refused {
                        said.push(Span::styled(format!(" · {why}"), Style::default().fg(Color::Red)));
                    }
                    said.push(Span::raw(" "));
                    let key = Style::default().fg(Color::Gray);
                    Foot {
                        said,
                        keys: vec![Span::raw(" "), Span::styled("q", key), Span::raw("/"), Span::styled("esc", key), Span::raw(" leave ")],
                    }
                } else {
                    Foot { said: vec![Span::raw(format!(" {} ", counted(s)))], keys: Vec::new() }
                };
                draw(s, Rect::new(area.x, area.y + h * i as u16, area.width, h), f.buffer_mut(), &mut borders, asked.tier, foot);
            }
        });
        if let Err(e) = frame {
            break Err(e.to_string());
        }
        if asked.tier.edges.is_some() {
            let set = rectangles(&borders, before.as_ref());
            let backend = terminal.backend_mut();
            if let Err(e) = backend.write_all(set.as_bytes()).and_then(|_| backend.flush()) {
                break Err(e.to_string());
            }
            before = Some(borders.clone());
        }
        // a second's tick keeps the status line's age true
        match event::poll(Duration::from_millis(400)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k))
                    if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
                        || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)) =>
                {
                    break Ok(());
                }
                Ok(_) => {}
                Err(e) => break Err(e.to_string()),
            },
            Ok(false) => {}
            Err(e) => break Err(e.to_string()),
        }
        let now = modified(&asked.path);
        if now != seen {
            seen = now;
            match summaries(&asked.path) {
                Ok((_, d)) => {
                    drawn = d;
                    refused = None;
                    read = Instant::now();
                }
                Err(why) => refused = Some(why),
            }
        }
    };
    ratatui::restore();
    result
}

/// What a card's foot says, in two segments as the head holds its title: what was read at
/// its leading end, and the keys at its trailing end when the card answers them (none, for
/// a card printed and left).
pub struct Foot {
    said: Vec<Span<'static>>,
    keys: Vec<Span<'static>>,
}

/// The fewest cells of edge between the foot's two segments, so they read as two.
const APART: usize = 3;

impl Foot {
    /// Its segments in `room` cells: the keys only where both fit `APART`; they go first,
    /// since what was read is the card's and the keys are the terminal's habit.
    fn segments(self, room: u16) -> (ratatui::text::Line<'static>, Option<ratatui::text::Line<'static>>) {
        use ratatui::text::Line as Words;
        let quiet = Style::default().fg(Color::DarkGray);
        let said = Words::from(self.said).style(quiet);
        let keys = Words::from(self.keys).style(quiet).right_aligned();
        let fits = keys.width() > 0 && said.width() + APART + keys.width() <= room as usize;
        (said, fits.then_some(keys))
    }
}

/// The rows a chart read, and those it left out.
fn counted(s: &Summary) -> String {
    let mut said = format!("{} rows", s.rows);
    if s.skipped > 0 {
        said += &format!(", {} left out (no place or no value)", s.skipped);
    }
    said
}

/// A card drawn: how tall it is, and the plot inside it where it has one.
pub struct Card {
    pub height: u16,
    #[cfg_attr(not(test), expect(dead_code, reason = "the tests read it: the room edges give the plot"))]
    pub plot: Option<Rect>,
}

/// A summary drawn into `area` as a card: its title in the card's top edge, its foot in
/// the bottom one, its legend inside, then its scale and marks, every mark on the scale's
/// one grid of rows. With edges, the frame is lines on the card's outer cells' edges and
/// the title and foot rows of their own inside it, a row of room from each line, so the
/// plot has the frame's columns.
/// The card is as tall as what it holds, at most `area`.
pub fn draw(s: &Summary, area: Rect, buf: &mut Buffer, borders: &mut Borders, tier: Tier, foot: Foot) -> Card {
    use ratatui::text::Line as Words;
    use ratatui::widgets::{Block, BorderType};
    let (said, keys) = foot.segments(area.width.saturating_sub(2));
    if let Some(edges) = tier.edges {
        let title = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
        buf.set_stringn(area.x + 2, area.y + 1, &s.title, area.width.saturating_sub(4) as usize, title);
        // a row of room under the frame's top line, the title's row and a row of room, then
        // the marks, a row of room, the foot's row and a row of room over the bottom line
        let room = Rect::new(area.x + 2, area.y + 3, area.width.saturating_sub(4), area.height.saturating_sub(6));
        let (end, plot) = if room.height < 6 || room.width < 20 { (room.y, None) } else { marks(s, room, buf, borders, tier) };
        let held = Rect::new(area.x, area.y, area.width, (end + 3 - area.y).min(area.height));
        let row = Rect::new(area.x + 1, held.bottom() - 2, area.width.saturating_sub(2), 1);
        said.render(row, buf);
        if let Some(keys) = keys {
            keys.render(row, buf);
        }
        frame(held, borders, tier, edges);
        return Card { height: held.height, plot };
    }
    let mut card = Block::bordered()
        .border_type(if tier.glyphs == Glyphs::Ascii { BorderType::Plain } else { BorderType::Rounded })
        .border_style(Style::default().fg(edge_color(tier)))
        .title(Words::from(format!(" {} ", s.title)).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
        .title_bottom(said);
    if let Some(keys) = keys {
        card = card.title_bottom(keys);
    }
    let inner = card.inner(area);
    // a cell of room inside the card's edge, on every side
    let room = Rect::new(inner.x + 2, inner.y + 1, inner.width.saturating_sub(4), inner.height.saturating_sub(2));
    let (end, plot) = if room.height < 6 || room.width < 20 { (room.y, None) } else { marks(s, room, buf, borders, tier) };
    let held = Rect::new(area.x, area.y, area.width, (end + 2 - area.y).min(area.height));
    card.render(held, buf);
    Card { height: held.height, plot }
}

/// The card's frame as lines inside its outer cells, on their outer edges; its corners
/// rounded where the terminal rounds them. The card stands on the terminal's own ground,
/// so a rounded corner cuts off nothing that needs an outside colour.
fn frame(card: Rect, borders: &mut Borders, tier: Tier, edges: Edges) {
    if card.width < 2 || card.height < 2 {
        return;
    }
    let edge = Stroke { width: FRAME, color: edge_color(tier), above: false };
    let (right, bottom) = (card.right() - 1, card.bottom() - 1);
    for x in card.left()..card.right() {
        borders.add(x, card.top(), Side::TOP, edge);
        borders.add(x, bottom, Side::BOTTOM, edge);
    }
    for y in card.top()..card.bottom() {
        borders.add(card.left(), y, Side::LEFT, edge);
        borders.add(right, y, Side::RIGHT, edge);
    }
    if edges.round {
        borders.round(card.left(), card.top(), Joint::TOP_LEFT);
        borders.round(right, card.top(), Joint::TOP_RIGHT);
        borders.round(right, bottom, Joint::BOTTOM_RIGHT);
        borders.round(card.left(), bottom, Joint::BOTTOM_LEFT);
    }
}

/// The legend, scale and marks of `s` in `area`; the row past the last one drawn is
/// returned, and the plot where there is one.
fn marks(s: &Summary, area: Rect, buf: &mut Buffer, borders: &mut Borders, tier: Tier) -> (u16, Option<Rect>) {
    let parts: Vec<&Summary> = if s.layers.is_empty() { vec![s] } else { s.layers.iter().collect() };
    let inks = Inks::of(&parts, tier);
    let y0 = legend(&parts, area, buf, tier, &inks);
    let body = Rect::new(area.x, y0, area.width, area.bottom().saturating_sub(y0));
    if s.kind == Kind::Figure && s.layers.is_empty() {
        return (figure(s, body, buf, tier, &inks), None);
    }
    if s.kind == Kind::Heat && s.layers.is_empty() {
        return heat(s, body, buf, tier, &inks);
    }
    let edged = tier.edges.is_some();
    // stacked bars and bars of shares; a heat as a layer is its stacked bars, as the app
    // draws it
    let stacked = |p: &Summary| Stack::piles(p.kind, p.stack);
    // the summary's scale, the same at every size; the plot as many rows as gives every
    // labelled step the same whole number of rows, so each labelled step's rule stands on
    // a row (with edges, on a row's foot) and each mark meets it there: every step where
    // the rows hold them, else every second, or fewer, a number of steps that divides the
    // scale's so the top is labelled too; under the plot the places, a row of room before
    // them where no edge is the axis
    let drawn = s.scale.expect("a plot's summary carries its scale");
    let room = body.height.saturating_sub(if edged { 1 } else { 2 }).max(3);
    let steps = drawn.steps().max(1) as u16;
    let every = (1..=steps).find(|k| steps % k == 0 && steps / k <= room - 1).expect("one step, lo to hi, takes two rows");
    let ruled_steps = steps / every;
    let per = (room - 1) / ruled_steps;
    let labels: Vec<String> = (0..=ruled_steps).map(|i| s.figure(drawn.tick((i * every) as u32))).collect();
    let left = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16 + 2;
    let plot = Rect::new(body.x + left, body.y, body.width.saturating_sub(left), ruled_steps * per + 1);
    if plot.width < 4 {
        return (body.y, None);
    }
    let scale = Scale { lo: drawn.lo, hi: drawn.hi, rows: plot.height };
    let quiet = Style::default().fg(Color::DarkGray);
    let rule = if tier.glyphs == Glyphs::Ascii { "-" } else { "─" };
    let ruled: Vec<u16> = (0..=ruled_steps).map(|i| plot.bottom() - 1 - i * per).collect();
    for (label, &row) in labels.iter().zip(&ruled) {
        // right-aligned against the plot, a faint rule across it through the row's middle
        // (with edges, the row's centre arms, drawn once the marks are)
        buf.set_string(plot.x - 2 - label.chars().count() as u16, row, label, quiet);
        if !edged {
            buf.set_string(plot.x, row, rule.repeat(plot.width as usize), Style::default().fg(grid_color(tier)));
        }
    }
    for part in &parts {
        match part.kind {
            Kind::Bars | Kind::Heat => bars(part, plot, buf, tier, &inks, &scale, stacked(part)),
            _ => lines(part, plot, buf, tier, &inks, &scale),
        }
    }
    if edged {
        // each rule through its row's middle, where its label stands and its value is: the
        // lowest is the axis, drawn over the bars standing on it; a rule through a bar is a
        // shade of the bar's colour drawn over it, as the rules through bars are without
        // edges
        for (i, &row) in ruled.iter().enumerate() {
            for x in plot.x..plot.right() {
                let stroke = match (i, through(&buf[(x, row)])) {
                    (0, _) => Stroke { width: AXIS, color: edge_color(tier), above: true },
                    (_, Some(bar)) => Stroke { width: RULE, color: shaded(bar, tier), above: true },
                    (_, None) => Stroke { width: RULE, color: grid_color(tier), above: false },
                };
                borders.add(x, row, Side::HORIZONTAL, stroke);
            }
        }
    } else if tier.glyphs != Glyphs::Ascii {
        // the rules seen through the bars, a shade of each bar's own colour
        for &row in &ruled {
            for x in plot.x..plot.right() {
                if let Some(cell) = buf.cell_mut((x, row))
                    && cell.symbol() == "█"
                {
                    let bar = cell.fg;
                    cell.set_symbol("─").set_fg(shaded(bar, tier)).set_bg(bar);
                }
            }
        }
    }
    (named_under(parts[0], plot, buf, tier), Some(plot))
}

/// Each place of `s` named under its mark's middle, as many as fit with a space between;
/// with edges right under the axis. The row past them is returned.
fn named_under(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier) -> u16 {
    let under = plot.bottom() + if tier.edges.is_some() { 0 } else { 1 };
    let names: Vec<String> = place_names(&s.places, s.axis);
    let mut free = plot.x;
    for (i, name) in names.iter().enumerate() {
        let w = name.chars().count() as u16;
        let (x0, bw) = slot(plot, names.len(), i);
        let at = (x0 + bw / 2).saturating_sub(w / 2).max(plot.x);
        if at >= free && at + w <= plot.right() {
            buf.set_string(at, under, name, Style::default().fg(Color::DarkGray));
            free = at + w + 2;
        }
    }
    under + 1
}

/// The legend, once a plot has more than one series: each series' mark and name in its
/// colour, as many to a line as fit, a name too long for a line of its own cut short; a
/// heat drawn alone has none, its series being its rows. The row the plot starts on is
/// returned, a row of room below the legend.
fn legend(parts: &[&Summary], area: Rect, buf: &mut Buffer, tier: Tier, inks: &Inks) -> u16 {
    if let [only] = parts
        && only.kind == Kind::Heat
    {
        return area.y;
    }
    let series: Vec<(Kind, &str, Shade)> = parts
        .iter()
        .flat_map(|p| p.series.iter().map(move |(n, sh)| (p.kind, if n.is_empty() { p.title.as_str() } else { n.as_str() }, *sh)))
        .collect();
    if series.len() < 2 {
        return area.y;
    }
    let (mut x, mut y) = (area.x, area.y);
    for (kind, name, shade) in series {
        let wide = (name.chars().count() + 2) as u16;
        if x > area.x && x + APART as u16 + wide > area.right() {
            (x, y) = (area.x, y + 1);
        } else if x > area.x {
            x += APART as u16;
        }
        let style = Style::default().fg(inks.color(shade));
        buf.set_string(x, y, swatch(kind, tier.glyphs), style);
        let room = area.right().saturating_sub(x + 2) as usize;
        let name: String = if name.chars().count() > room {
            name.chars().take(room.saturating_sub(1)).chain([if tier.glyphs == Glyphs::Ascii { '~' } else { '…' }]).collect()
        } else {
            name.to_owned()
        };
        buf.set_string(x + 2, y, &name, style);
        x += 2 + name.chars().count() as u16;
    }
    y + 2
}

/// A series' mark in the legend: what its kind draws, in a cell.
fn swatch(kind: Kind, glyphs: Glyphs) -> &'static str {
    match (kind, glyphs == Glyphs::Ascii) {
        (Kind::Bars | Kind::Heat | Kind::Figure, false) => "█",
        (Kind::Bars | Kind::Heat | Kind::Figure, true) => "#",
        (Kind::Area, false) => "▄",
        (Kind::Area, true) => "=",
        (Kind::Line, false) => "─",
        (Kind::Line, true) => "-",
        (Kind::Steps, false) => "┘",
        (Kind::Steps, true) => "_",
        (Kind::Points, false) => "•",
        (Kind::Points, true) => "*",
    }
}

/// Where a value stands on a plot of `rows` rows, in eighths of a row from its foot: `lo`
/// on the foot row's middle, `hi` on the top row's, so a step's rule (drawn through a
/// row's middle, as a glyph or as the row's centre arms) is where its value is, for a
/// bar's top and a line's point alike.
struct Scale {
    lo: f64,
    hi: f64,
    rows: u16,
}

impl Scale {
    fn eighths(&self, v: f64) -> i64 {
        (4.0 + (v - self.lo) / (self.hi - self.lo) * (self.rows as f64 - 1.0) * 8.0).round() as i64
    }

    /// The bounds a canvas spans so its rows' middles are `lo` and `hi`.
    fn bounds(&self) -> [f64; 2] {
        let half = (self.hi - self.lo) / ((self.rows as f64 - 1.0) * 2.0);
        [self.lo - half, self.hi + half]
    }
}

/// Place `p` of `n` along a plot: its mark's first column and width, the same for its bar
/// and its name.
fn slot(plot: Rect, n: usize, p: usize) -> (u16, u16) {
    let band = plot.width as f64 / n.max(1) as f64;
    let width = (band * 0.7).max(1.0) as u16;
    (plot.x + (band * p as f64 + (band - width as f64) / 2.0) as u16, width)
}

/// The rules' colour: faint against the terminal's ground.
fn grid_color(tier: Tier) -> Color {
    match tier.colors {
        Colors::True => Color::Rgb(58, 58, 64),
        Colors::Indexed => Color::Indexed(237),
        Colors::Sixteen => Color::DarkGray,
    }
}

/// The card's edge: a step past the rules, so the card holds them.
fn edge_color(tier: Tier) -> Color {
    match tier.colors {
        Colors::True => Color::Rgb(84, 84, 92),
        Colors::Indexed => Color::Indexed(240),
        Colors::Sixteen => Color::DarkGray,
    }
}

/// The bar a cell holds across its middle, where a rule runs: an eighth block's ink when it
/// rises past the middle, else its ground, the stacked bar over it, where it has one.
fn through(cell: &ratatui::buffer::Cell) -> Option<Color> {
    match cell.symbol() {
        "▅" | "▆" | "▇" | "█" => Some(cell.fg),
        "▁" | "▂" | "▃" | "▄" => (cell.bg != Color::Reset).then_some(cell.bg),
        _ => None,
    }
}

/// A bar's colour a step darker, a rule seen through it.
fn shaded(bar: Color, tier: Tier) -> Color {
    match (bar, tier.colors) {
        (Color::Rgb(r, g, b), _) => Color::Rgb((r as u16 * 7 / 10) as u8, (g as u16 * 7 / 10) as u8, (b as u16 * 7 / 10) as u8),
        _ => Color::Black,
    }
}

/// Places as a person reads them: dates by their day (`14 Aug`), every one a month's first
/// day by its month (`Jan 26`), every one a year's by its year; anything else as written.
fn place_names(places: &[String], axis: &str) -> Vec<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let parts = |p: &str| -> Option<(String, usize, String)> {
        let mut it = p.split('-');
        let (y, m, d) = (it.next()?, it.next()?.parse::<usize>().ok()?, it.next()?);
        (it.next().is_none() && (1..=12).contains(&m)).then(|| (y.to_owned(), m, d.to_owned()))
    };
    if axis != "date" || !places.iter().all(|p| parts(p).is_some()) {
        return places.to_vec();
    }
    let years = places.iter().all(|p| p.ends_with("-01-01"));
    let months = places.iter().all(|p| p.ends_with("-01"));
    places
        .iter()
        .map(|p| {
            let (y, m, d) = parts(p).expect("every place a date, checked above");
            if years {
                y
            } else if months {
                format!("{} {}", MONTHS[m - 1], &y[y.len().saturating_sub(2)..])
            } else {
                format!("{} {}", d.trim_start_matches('0'), MONTHS[m - 1])
            }
        })
        .collect()
}

/// Bars in eighths along the value: where two stacked series meet in one cell, the lower
/// is the glyph's ink and the upper its ground, so a stack has no gap.
fn bars(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier, inks: &Inks, scale: &Scale, stacked: bool) {
    let eighths = ["", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let units = |v: f64| scale.eighths(v);
    let zero = units(0.0);
    let places = s.places.len().max(1);
    let series = s.series.len().max(1);
    for (p, row) in s.values.iter().enumerate() {
        // each series' span along the value, in eighths from the plot's foot
        let mut spans: Vec<(i64, i64, usize)> = Vec::new();
        let mut up = zero;
        for (k, v) in row.iter().enumerate() {
            let Some(v) = v.as_deref().and_then(|t| t.parse::<f64>().ok()) else { continue };
            if stacked {
                let to = up + units(v) - zero;
                spans.push((up.min(to), up.max(to), k));
                up = to;
            } else {
                spans.push((zero.min(units(v)), zero.max(units(v)), k));
            }
        }
        let (x0, width) = slot(plot, places, p);
        let columns: Vec<(u16, u16, Vec<(i64, i64, usize)>)> = if stacked || series == 1 {
            vec![(x0, width, spans)]
        } else {
            let w = (width / series as u16).max(1);
            spans.iter().map(|sp| (x0 + w * sp.2 as u16, w, vec![*sp])).collect()
        };
        for (x, w, spans) in columns {
            for r in 0..plot.height {
                // this cell's eighths, from its foot
                let foot = (plot.height - 1 - r) as i64 * 8;
                let covering: Vec<(i64, i64, usize)> = spans
                    .iter()
                    .filter_map(|&(a, b, k)| {
                        let (a, b) = (a.max(foot), b.min(foot + 8));
                        (a < b).then_some((a - foot, b - foot, k))
                    })
                    .collect();
                if covering.is_empty() {
                    continue;
                }
                let lower = covering.iter().min_by_key(|c| c.0).unwrap();
                let upper = covering.iter().max_by_key(|c| c.1).unwrap();
                let (glyph, fg, bg) = if tier.glyphs == Glyphs::Ascii {
                    ("#", lower.2, None)
                } else if lower.0 == 0 {
                    (eighths[lower.1 as usize], lower.2, (upper.2 != lower.2).then_some(upper.2))
                } else {
                    // a span that starts above the cell's foot, as every bar does on zero's
                    // row (zero stands on the row's middle, where its rule is): the cell's
                    // upper part, in the nearest top block there is
                    (
                        if lower.0 <= 2 {
                            "█"
                        } else if lower.0 <= 5 {
                            "▀"
                        } else {
                            "▔"
                        },
                        lower.2,
                        None,
                    )
                };
                for dx in 0..w {
                    if let Some(cell) = buf.cell_mut((x + dx, plot.y + r)) {
                        cell.set_symbol(glyph);
                        if fg != usize::MAX {
                            cell.set_fg(inks.color(s.series[fg].1));
                        }
                        if let Some(b) = bg {
                            cell.set_bg(inks.color(s.series[b].1));
                        }
                    }
                }
            }
        }
    }
}

/// Lines, areas, steps and points on a canvas of the tier's finest marks.
fn lines(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier, inks: &Inks, scale: &Scale) {
    let marker = match tier.glyphs {
        Glyphs::Fine => Marker::Braille,
        Glyphs::Blocks => Marker::HalfBlock,
        Glyphs::Ascii => Marker::Dot,
    };
    let n = s.places.len().max(2) as f64;
    Canvas::default()
        .marker(marker)
        .x_bounds([-0.5, n - 0.5])
        .y_bounds(scale.bounds())
        .paint(|ctx| {
            for (k, (_, shade)) in s.series.iter().enumerate() {
                let c = inks.color(*shade);
                let pts: Vec<(f64, f64)> = s
                    .values
                    .iter()
                    .enumerate()
                    .filter_map(|(p, row)| row[k].as_deref().and_then(|t| t.parse::<f64>().ok()).map(|v| (p as f64, v)))
                    .collect();
                match s.kind {
                    Kind::Points => ctx.draw(&Points { coords: &pts, color: c }),
                    _ => {
                        for w in pts.windows(2) {
                            let ((x1, y1), (x2, y2)) = (w[0], w[1]);
                            if s.kind == Kind::Steps {
                                ctx.draw(&Segment { x1, y1, x2, y2: y1, color: c });
                                ctx.draw(&Segment { x1: x2, y1, x2, y2, color: c });
                            } else {
                                ctx.draw(&Segment { x1, y1, x2, y2, color: c });
                            }
                            if s.kind == Kind::Area {
                                let steps = 12;
                                for i in 0..=steps {
                                    let t = i as f64 / steps as f64;
                                    let (x, y) = (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t);
                                    ctx.draw(&Segment { x1: x, y1: 0.0f64.max(scale.lo), x2: x, y2: y, color: c });
                                }
                            }
                        }
                        if pts.len() == 1 {
                            ctx.draw(&Points { coords: &pts, color: c });
                        }
                    }
                }
            }
        })
        .render(plot, buf);
}

/// A cell a pair of places, its depth the value: shades of one tint, each row (a series)
/// named at its left as a scale's labels stand, each place under its column. The row past
/// the names is returned, and the grid.
fn heat(s: &Summary, body: Rect, buf: &mut Buffer, tier: Tier, inks: &Inks) -> (u16, Option<Rect>) {
    let all: Vec<f64> = s.values.iter().flatten().filter_map(|v| v.as_deref().and_then(|t| t.parse::<f64>().ok())).collect();
    let (lo, hi) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let tint = s.series.first().map_or(Shade::Quiet, |x| x.1);
    let cols = s.places.len().max(1) as u16;
    let rows = s.series.len().max(1) as u16;
    let left = s.series.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0) as u16 + 2;
    let room = body.height.saturating_sub(if tier.edges.is_some() { 1 } else { 2 });
    let (cw, ch) = ((body.width.saturating_sub(left) / cols).max(1), (room / rows).clamp(1, 3));
    let plot = Rect::new(body.x + left, body.y, cw * cols, ch * rows);
    if plot.width < 4 {
        return (body.y, None);
    }
    for (k, (name, _)) in s.series.iter().enumerate() {
        let w = name.chars().count() as u16;
        buf.set_string(plot.x - 2 - w, plot.y + k as u16 * ch + (ch - 1) / 2, name, Style::default().fg(Color::DarkGray));
    }
    let shades = if tier.glyphs == Glyphs::Ascii { [".", ":", "*", "#"] } else { ["░", "▒", "▓", "█"] };
    for (p, row) in s.values.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            let Some(v) = v.as_deref().and_then(|t| t.parse::<f64>().ok()) else { continue };
            let depth = if hi > lo { (v - lo) / (hi - lo) } else { 1.0 };
            let glyph = shades[((depth * 3.0).round() as usize).min(3)];
            for dy in 0..ch {
                for dx in 0..cw {
                    if let Some(cell) = buf.cell_mut((plot.x + p as u16 * cw + dx, plot.y + k as u16 * ch + dy)) {
                        cell.set_symbol(glyph).set_fg(inks.color(tint));
                    }
                }
            }
        }
    }
    (named_under(s, plot, buf, tier), Some(plot))
}

/// One number, its change from the place before, and its run as a sparkline.
fn figure(s: &Summary, area: Rect, buf: &mut Buffer, tier: Tier, inks: &Inks) -> u16 {
    let run: Vec<f64> = s.values.iter().filter_map(|r| r.first().cloned().flatten().and_then(|t| t.parse::<f64>().ok())).collect();
    let Some(&now) = run.last() else { return area.y };
    buf.set_string(area.x, area.y, s.figure(now), Style::default().add_modifier(Modifier::BOLD));
    if run.len() > 1 {
        let change = run[run.len() - 1] - run[run.len() - 2];
        let (sign, c) = if change >= 0.0 { ("+", Color::Green) } else { ("", Color::Red) };
        buf.set_string(area.x, area.y + 1, format!("{sign}{}", s.figure(change)), Style::default().fg(c));
        let bars = if tier.glyphs == Glyphs::Ascii {
            ["_", "_", "-", "-", "=", "=", "#", "#"]
        } else {
            ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"]
        };
        let (lo, hi) = run.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        let tint = inks.color(s.series.first().map_or(Shade::Quiet, |x| x.1));
        for (i, v) in run.iter().rev().take(area.width as usize).rev().enumerate() {
            let level = if hi > lo { ((v - lo) / (hi - lo) * 7.0).round() as usize } else { 7 };
            buf.set_string(area.x + i as u16, area.y + 2, bars[level], Style::default().fg(tint));
        }
        return area.y + 3;
    }
    area.y + 1
}

/// The colours a chart's series are drawn in at a tier, one to each shade they wear, what
/// the marks and the legend alike are drawn in. True colour draws a shade as it is; the
/// 256's cube and the sixteen hold fewer, so each shade takes the nearest the tier holds
/// that no other shade of the chart takes, the closest pairs chosen first (ties in the
/// series' order): two series stay two on screen. Only shades past the colours a tier
/// holds share, each its nearest.
struct Inks(Vec<(u32, Color)>);

impl Inks {
    fn of(parts: &[&Summary], tier: Tier) -> Inks {
        let mut shades: Vec<u32> = Vec::new();
        for (_, shade) in parts.iter().flat_map(|p| &p.series) {
            if !shades.contains(&rgb(*shade)) {
                shades.push(rgb(*shade));
            }
        }
        let held: Vec<(Color, u32)> = match tier.colors {
            Colors::True => return Inks(shades.iter().map(|&c| (c, Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8))).collect()),
            // the cube's six levels a channel
            Colors::Indexed => {
                (0..216u32).map(|i| (Color::Indexed(16 + i as u8), (i / 36 * 51) << 16 | (i / 6 % 6 * 51) << 8 | i % 6 * 51)).collect()
            }
            // the twelve of the sixteen a series is drawn in, by their usual values
            Colors::Sixteen => [
                (Color::Red, 0xcd3131),
                (Color::Green, 0x0dbc79),
                (Color::Yellow, 0xe5e510),
                (Color::Blue, 0x2472c8),
                (Color::Magenta, 0xbc3fbc),
                (Color::Cyan, 0x11a8cd),
                (Color::Gray, 0xe5e5e5),
                (Color::DarkGray, 0x666666),
                (Color::LightRed, 0xf14c4c),
                (Color::LightGreen, 0x23d18b),
                (Color::LightYellow, 0xf5f543),
                (Color::LightMagenta, 0xd670d6),
            ]
            .to_vec(),
        };
        let apart = |a: u32, b: u32| {
            let d = |shift: u32| ((a >> shift) & 0xff) as i32 - ((b >> shift) & 0xff) as i32;
            d(16) * d(16) + d(8) * d(8) + d(0) * d(0)
        };
        let mut pairs: Vec<(i32, usize, usize)> =
            shades.iter().enumerate().flat_map(|(s, &c)| held.iter().enumerate().map(move |(h, &(_, t))| (apart(c, t), s, h))).collect();
        pairs.sort();
        let mut taken: Vec<Option<usize>> = vec![None; shades.len()];
        let mut used = vec![false; held.len()];
        for (_, s, h) in pairs {
            if taken[s].is_none() && !used[h] {
                taken[s] = Some(h);
                used[h] = true;
            }
        }
        let nearest = |c: u32| (0..held.len()).min_by_key(|&h| apart(c, held[h].1)).expect("a tier holds colours");
        Inks(shades.iter().zip(taken).map(|(&c, h)| (c, held[h.unwrap_or_else(|| nearest(c))].0)).collect())
    }

    fn color(&self, shade: Shade) -> Color {
        self.0.iter().find(|(c, _)| *c == rgb(shade)).expect("every series' shade is inked").1
    }
}

/// A shade as `0xrrggbb`, on a dark ground.
fn rgb(shade: Shade) -> u32 {
    u32::from_str_radix(&shade.hex(true)[1..], 16).expect("a shade is #rrggbb")
}

/// A named colour's place among the sixteen: 0 to 7, and the bright ones 8 to 15.
fn sixteen(named: Color) -> u8 {
    match named {
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        _ => 15,
    }
}

/// How thick a line is, in 256ths of a cell's width (the protocol's unit; the terminal
/// draws at least a device pixel): the frame and the axis a hairline, a rule half of one,
/// so it is the faintest line the terminal draws.
const FRAME: u8 = 16;
const AXIS: u8 = 16;
const RULE: u8 = 8;

/// One line's look: its thickness, its colour, and whether it is drawn over the cell's
/// text or under it. Every line here is in the protocol's solid style, an edge's inside its
/// cell, flush with the edge, and a centre arm on the cell's centreline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    width: u8,
    color: Color,
    above: bool,
}

impl Stroke {
    /// `WIDTH;COLOR;PLACEMENT;LAYER`: the colour as `rrggbb`, a palette index `pN`, or the
    /// terminal's foreground `fg` for a colour that is the terminal's own.
    fn fields(&self) -> String {
        let color = match self.color {
            Color::Rgb(r, g, b) => format!("{r:02x}{g:02x}{b:02x}"),
            Color::Indexed(i) => format!("p{i}"),
            Color::Reset => "fg".to_owned(),
            named => format!("p{}", sixteen(named)),
        };
        format!("{};{color};0;{}", self.width, u8::from(self.above))
    }
}

/// The protocol's segments: a cell's four edges, each its two halves, and the two arms of
/// its horizontal centreline, a line through its middle.
struct Side;

impl Side {
    const TOP: u16 = 0b11;
    const RIGHT: u16 = 0b1100;
    const BOTTOM: u16 = 0b11_0000;
    const LEFT: u16 = 0b1100_0000;
    const HORIZONTAL: u16 = 0b11_0000_0000;
}

/// The protocol's joints: a cell's corners, the quadrant of each inside the cell.
struct Joint;

impl Joint {
    const TOP_LEFT: u8 = 1;
    const TOP_RIGHT: u8 = 2;
    const BOTTOM_RIGHT: u8 = 4;
    const BOTTOM_LEFT: u8 = 8;
}

/// A cell's borders: its segments, each group under one stroke, and its rounded joints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Border {
    strokes: Vec<(u16, Stroke)>,
    round: u8,
}

impl Border {
    fn is_empty(&self) -> bool {
        self.strokes.is_empty() && self.round == 0
    }

    /// The segments of `mask` given `stroke`, taken from any other stroke they had.
    fn add(&mut self, mask: u16, stroke: Stroke) {
        for s in &mut self.strokes {
            s.0 &= !mask;
        }
        self.strokes.retain(|s| s.0 != 0);
        match self.strokes.iter_mut().find(|s| s.1 == stroke) {
            Some(s) => s.0 |= mask,
            None => self.strokes.push((mask, stroke)),
        }
    }

    /// The pen that draws these borders, as set on an empty pen: one `pen` a stroke, and
    /// `pen-round` for the rounded joints.
    fn pen(&self) -> String {
        let mut said: String = self.strokes.iter().map(|(mask, s)| dcs(&format!("pen;{mask};{}", s.fields()))).collect();
        if self.round != 0 {
            said += &dcs(&format!("pen-round;{}", self.round));
        }
        said
    }
}

/// One command of the cell border protocol: `DCS 1 + b cell-border;… ST`.
fn dcs(body: &str) -> String {
    format!("\x1bP1+bcell-border;{body}\x1b\\")
}

/// What ratatui's Buffer has no place for: the borders each cell of an area carries.
#[derive(Clone, Debug)]
pub struct Borders {
    area: Rect,
    cells: Vec<Border>,
}

impl Borders {
    pub fn new(area: Rect) -> Borders {
        Borders { area, cells: vec![Border::default(); area.area() as usize] }
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        let a = self.area;
        (x >= a.left() && x < a.right() && y >= a.top() && y < a.bottom())
            .then(|| (y - a.y) as usize * a.width as usize + (x - a.x) as usize)
    }

    /// The borders of the cell at `x`, `y`; none outside the area.
    fn at(&self, x: u16, y: u16) -> &Border {
        static NONE: Border = Border { strokes: Vec::new(), round: 0 };
        self.index(x, y).map_or(&NONE, |i| &self.cells[i])
    }

    fn add(&mut self, x: u16, y: u16, mask: u16, stroke: Stroke) {
        if let Some(i) = self.index(x, y) {
            self.cells[i].add(mask, stroke);
        }
    }

    fn round(&mut self, x: u16, y: u16, joints: u8) {
        if let Some(i) = self.index(x, y) {
            self.cells[i].round |= joints;
        }
    }
}

/// The borders of a frame held on the screen, set on the cells ratatui wrote: from the top
/// left, every run of cells along a row that carries the same borders set as one rectangle.
/// When they differ from the frame `before`, the screen's borders are cleared first, so a
/// cell that carries none any more is left with none.
fn rectangles(borders: &Borders, before: Option<&Borders>) -> String {
    let a = borders.area;
    let mut said = String::new();
    if before.is_none_or(|b| b.area != a || b.cells != borders.cells) {
        said += &format!("\x1b[{};{}H", a.y + 1, a.x + 1);
        said += &dcs(&format!("clear;4095;{};{}", a.height, a.width));
        said += &dcs(&format!("square;255;{};{}", a.height, a.width));
    }
    for y in a.top()..a.bottom() {
        let mut x = a.left();
        while x < a.right() {
            let b = borders.at(x, y);
            let run = (x..a.right()).take_while(|&x2| borders.at(x2, y) == b).count() as u16;
            if !b.is_empty() {
                said += &format!("\x1b[{};{}H", y + 1, x + 1);
                for (mask, s) in &b.strokes {
                    said += &dcs(&format!("set;{mask};{};1;{run}", s.fields()));
                }
                if b.round != 0 {
                    said += &dcs(&format!("round;{};1;{run}", b.round));
                }
            }
            x += run;
        }
    }
    said
}

/// A buffer printed inline: each row its cells with their colours, the colours reset at
/// its end. A cell with borders is printed with the pen that draws them; since SGR 0
/// clears the pen, the pen is set again after each reset, and cleared before every line
/// ends, so neither the next line scrolled in nor the shell's prompt takes it.
fn print(buf: &Buffer, borders: &Borders, tier: Tier, out: &mut impl Write) -> io::Result<()> {
    let sgr = |c: Color, ground: bool| -> Option<String> {
        let base = if ground { 40 } else { 30 };
        match c {
            Color::Reset => None,
            Color::Rgb(r, g, b) => Some(format!("{};2;{r};{g};{b}", base + 8)),
            Color::Indexed(i) => Some(format!("{};5;{i}", base + 8)),
            named => {
                let n = sixteen(named);
                Some(format!("{}", if n >= 8 { base + 60 + (n - 8) } else { base + n }))
            }
        }
    };
    let colored = io::stdout().is_terminal() || tier.colors != Colors::Sixteen;
    let area = buf.area;
    let empty = Border::default();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        // what the terminal's pen holds
        let mut pen = &empty;
        for x in area.left()..area.right() {
            let cell = &buf[(x, y)];
            let wanted = borders.at(x, y);
            if wanted != pen {
                if !pen.is_empty() {
                    line += "\x1b[0m";
                }
                line += &wanted.pen();
                pen = wanted;
            }
            let codes: Vec<String> = [sgr(cell.fg, false), sgr(cell.bg, true)].into_iter().flatten().collect();
            let bold = cell.modifier.contains(Modifier::BOLD);
            if colored && (!codes.is_empty() || bold) {
                line += &format!("\x1b[{}{}m{}\x1b[0m", if bold { "1;" } else { "" }, codes.join(";"), cell.symbol());
                pen = &empty;
            } else {
                line += cell.symbol();
            }
        }
        if !pen.is_empty() {
            line += "\x1b[0m";
        }
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FINE: Tier = Tier { glyphs: Glyphs::Fine, colors: Colors::True, edges: None };
    const EDGED: Tier = Tier { edges: Some(Edges { round: true }), ..FINE };
    const INDEXED: Tier = Tier { colors: Colors::Indexed, ..FINE };
    const SIXTEEN: Tier = Tier { colors: Colors::Sixteen, ..FINE };

    /// Chart `n` of a case in the suite's `tests/charts`.
    fn summary(case: &str, n: usize) -> Summary {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/charts").join(format!("{case}.cells"));
        let bytes = fs::read(&path).expect("the case is in the suite");
        let doc = cells::read_with(bytes, &mut Disk::new(&path), 1).expect("the case reads");
        doc.summary(&doc.charts()[n - 1])
    }

    /// A chart drawn as `cells chart` draws it at 80 columns: the card, its cells, its
    /// borders and the bytes printed.
    fn drawn(s: &Summary, tier: Tier) -> (Card, Buffer, Borders, String) {
        let area = Rect::new(0, 0, 80, 21);
        let mut buf = Buffer::empty(area);
        let mut borders = Borders::new(area);
        let card = draw(s, area, &mut buf, &mut borders, tier, Foot { said: vec![Span::raw(" 4 rows ")], keys: Vec::new() });
        buf.resize(Rect::new(0, 0, 80, card.height));
        let mut out = Vec::new();
        print(&buf, &borders, tier, &mut out).expect("a Vec takes every byte");
        (card, buf, borders, String::from_utf8(out).expect("the chart is UTF-8"))
    }

    /// Every command a line holds, `DCS 1 + b cell-border;…` up to its `ST`, and the bytes
    /// after the last one.
    fn commands(line: &str) -> (Vec<&str>, &str) {
        let mut found = Vec::new();
        let mut rest = line;
        while let Some(at) = rest.find("\x1bP") {
            let from = &rest[at + 2..];
            let end = from.find("\x1b\\").unwrap_or_else(|| panic!("a DCS with no ST: {from:?}"));
            found.push(&from[..end]);
            rest = &from[end + 2..];
        }
        (found, rest)
    }

    /// A command as the spec writes it: `1+b`, the namespace, an operation it names, its
    /// positional fields in range, no more than 256 bytes.
    fn well_formed(dcs: &str) {
        let body = dcs.strip_prefix("1+b").unwrap_or_else(|| panic!("not version 1, `+b`: {dcs:?}"));
        assert!(body.len() <= 256, "a payload over 256 bytes is ignored: {body:?}");
        let fields: Vec<&str> = body.split(';').collect();
        assert_eq!(fields[0], "cell-border", "{body:?}");
        let number = |f: &str, lo: u32, hi: u32| {
            let n: u32 = f.parse().unwrap_or_else(|_| panic!("{f:?} is no decimal in {body:?}"));
            assert!((lo..=hi).contains(&n), "{n} is outside {lo}..={hi} in {body:?}");
        };
        let stroke = |f: &[&str]| {
            number(f[0], 1, 4095);
            number(f[1], 1, 64);
            let color = f[2];
            let hex = color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit());
            let palette = color.strip_prefix('p').is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n <= 255));
            assert!(hex || palette || color == "fg", "{color:?} is no colour in {body:?}");
            assert!(matches!(f[3], "0" | "1"), "placement {:?} in {body:?}", f[3]);
            assert!(matches!(f[4], "0" | "1"), "layer {:?} in {body:?}", f[4]);
        };
        match fields[1] {
            "pen" => {
                assert_eq!(fields.len(), 7, "{body:?}");
                stroke(&fields[2..]);
            }
            "pen-round" => {
                assert_eq!(fields.len(), 3, "{body:?}");
                number(fields[2], 1, 255);
            }
            "set" => {
                assert_eq!(fields.len(), 9, "{body:?}");
                stroke(&fields[2..7]);
                number(fields[7], 1, 65535);
                number(fields[8], 1, 65535);
            }
            "clear" => {
                assert_eq!(fields.len(), 5, "{body:?}");
                number(fields[2], 1, 4095);
                number(fields[3], 1, 65535);
                number(fields[4], 1, 65535);
            }
            "round" | "square" => {
                assert_eq!(fields.len(), 5, "{body:?}");
                number(fields[2], 1, 255);
                number(fields[3], 1, 65535);
                number(fields[4], 1, 65535);
            }
            op => panic!("{op:?} is no operation of the protocol: {body:?}"),
        }
    }

    /// Without edges a chart carries no border and prints no DCS.
    #[test]
    fn no_edges_no_commands() {
        for (case, n) in [("bars-by-month", 1), ("line-area-and-steps", 1), ("heat", 1), ("figure", 1)] {
            let (_, _, borders, bytes) = drawn(&summary(case, n), FINE);
            assert!(borders.cells.iter().all(Border::is_empty), "{case} carries borders without edges");
            assert!(!bytes.contains("\x1bP"), "{case} prints a DCS without edges");
        }
    }

    /// With edges every command is well-formed, and every line that sets the pen resets it
    /// with SGR 0 before it ends, so the newline's scroll and the shell take no borders.
    #[test]
    fn the_pen_is_well_formed_and_cleared() {
        for (case, n) in [("bars-by-month", 1), ("line-area-and-steps", 1), ("layers-by-name", 3), ("heat", 1), ("figure", 1)] {
            let (_, _, _, bytes) = drawn(&summary(case, n), EDGED);
            let mut pens = 0;
            for line in bytes.lines() {
                let (found, after) = commands(line);
                for dcs in &found {
                    well_formed(dcs);
                    assert!(dcs.starts_with("1+bcell-border;pen"), "{case}: printing sets only the pen: {dcs:?}");
                }
                if !found.is_empty() {
                    assert!(after.contains("\x1b[0m"), "{case}: a line ends with its pen held: {line:?}");
                }
                pens += found.len();
            }
            assert!(pens > 0, "{case} sets no pen with edges");
        }
    }

    /// With edges the frame takes no cell: the plot has the frame's two columns, and the
    /// rows the frame gives back stand between the frame's lines and the title and the
    /// foot, so no text touches a line; nothing is drawn in box drawing.
    #[test]
    fn edges_give_the_plot_the_frames_room() {
        for (case, n) in [("bars-by-month", 1), ("line-area-and-steps", 1)] {
            let s = summary(case, n);
            let (boxed, _, _, _) = drawn(&s, FINE);
            let (edged, buf, borders, _) = drawn(&s, EDGED);
            let (boxed, edged_plot) = (boxed.plot.expect("a plot"), edged.plot.expect("a plot"));
            assert_eq!(edged_plot.width, boxed.width + 2, "{case}: the frame's columns are the plot's");
            assert_eq!(edged_plot.height, boxed.height, "{case}: {} rows edged, {} boxed", edged_plot.height, boxed.height);
            assert!(edged.height <= 21, "{case}: the card is taller than its area");
            for cell in buf.content() {
                assert!(!"─│╭╮╰╯┌┐└┘".contains(cell.symbol()) || cell.symbol().is_empty(), "{case}: a box glyph with edges");
            }
            // a row of room between each frame line and the text nearest it
            let text = |y: u16| (1..79).any(|x| !buf[(x, y)].symbol().trim().is_empty());
            assert!(!text(0) && text(1), "{case}: the title is not a row under the frame's top");
            assert!(!text(edged.height - 1) && text(edged.height - 2), "{case}: the foot is not a row over the frame's bottom");
            // the axis through the plot's foot row's middle, each rule through its row's
            // middle, where its label stands
            let axis = edged_plot.bottom() - 1;
            for x in edged_plot.left()..edged_plot.right() {
                let arms = borders.at(x, axis).strokes.iter().find(|(m, _)| m & Side::HORIZONTAL == Side::HORIZONTAL);
                assert!(arms.is_some_and(|(_, s)| s.width == AXIS && s.above), "{case}: no axis at {x}");
            }
            assert!(
                borders.cells.iter().all(|b| b.strokes.iter().all(|(m, s)| s.width == FRAME || m & Side::BOTTOM == 0)),
                "{case}: a rule on a row's foot"
            );
            // the frame on the card's outer cells, its corners rounded
            assert_eq!(borders.at(0, 0).round, Joint::TOP_LEFT);
            assert_eq!(borders.at(79, edged.height - 1).round, Joint::BOTTOM_RIGHT);
            assert!(borders.at(40, 0).strokes.iter().any(|(m, _)| *m & Side::TOP == Side::TOP));
            assert!(borders.at(0, 10).strokes.iter().any(|(m, _)| *m & Side::LEFT == Side::LEFT));
        }
        // the numbers for one, at 80 by 21: bars by month, 66 by 13 boxed, 68 by 13 edged
        let s = summary("bars-by-month", 1);
        let (boxed, edged) = (drawn(&s, FINE).0.plot.unwrap(), drawn(&s, EDGED).0.plot.unwrap());
        assert_eq!((boxed.width, boxed.height), (66, 13));
        assert_eq!((edged.width, edged.height), (68, 13));
    }

    /// The scale is the summary's at every size and in both tiers: the plot's foot is its
    /// `lo` and its top its `hi`, and a card too short to label every step labels fewer of
    /// the same steps, never steps of its own.
    #[test]
    fn the_scale_is_the_datas_at_every_size() {
        for (case, n) in [("bars-by-month", 1), ("date-trunc-units", 1), ("stack-modes", 1), ("stack-modes", 3), ("line-area-and-steps", 1)]
        {
            let s = summary(case, n);
            let scale = s.scale.expect("a plot's scale");
            let ticks: Vec<String> = (0..=scale.steps()).map(|i| s.figure(scale.tick(i))).collect();
            for tier in [FINE, EDGED] {
                // the shortest card that holds a plot, its frame and its rows of room
                let short = if tier.edges.is_some() { 12 } else { 10 };
                for (width, height) in [(80, 21), (80, 14), (40, short + 2), (40, short + 1), (30, short)] {
                    let area = Rect::new(0, 0, width, height);
                    let mut buf = Buffer::empty(area);
                    let card = draw(&s, area, &mut buf, &mut Borders::new(area), tier, Foot { said: Vec::new(), keys: Vec::new() });
                    let plot = card.plot.unwrap_or_else(|| panic!("{case} at {width}x{height} ({tier:?}) has no plot"));
                    let label = |y: u16| (0..plot.x).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_matches(['│', ' ']).to_owned();
                    let labels: Vec<(u16, String)> =
                        (plot.top()..plot.bottom()).map(|y| (y, label(y))).filter(|(_, l)| !l.is_empty()).collect();
                    let at = format!("{case} at {width}x{height} ({tier:?})");
                    assert!(labels.iter().all(|(_, l)| ticks.contains(l)), "{at}: {labels:?} are not all of {ticks:?}");
                    assert_eq!(labels.first().map(|l| (l.0, l.1.as_str())), Some((plot.top(), ticks.last().unwrap().as_str())), "{at}");
                    assert_eq!(labels.last().map(|l| (l.0, l.1.as_str())), Some((plot.bottom() - 1, ticks[0].as_str())), "{at}");
                }
            }
            // at full height, every step
            let (c, buf, _, _) = drawn(&s, FINE);
            let plot = c.plot.expect("a plot");
            let shown = (plot.top()..plot.bottom()).filter(|&y| (1..plot.x).any(|x| !buf[(x, y)].symbol().trim().is_empty())).count();
            assert_eq!(shown, ticks.len(), "{case}: every step labelled at 80x21");
        }
    }

    /// A percent stack draws the summary's shares on the summary's 0% to 100%, in both
    /// tiers: every place's bar stands from the foot to the top, its series stacked.
    #[test]
    fn shares_fill_the_scale() {
        let s = summary("stack-modes", 3);
        assert_eq!(s.figure(s.scale.expect("a plot's scale").hi), "100%");
        for tier in [FINE, EDGED] {
            let (c, buf, _, _) = drawn(&s, tier);
            let plot = c.plot.expect("a plot");
            for p in 0..s.places.len() {
                let (x0, w) = slot(plot, s.places.len(), p);
                let mid = x0 + w / 2;
                let filled = (plot.top()..plot.bottom()).all(|y| !buf[(mid, y)].symbol().trim().is_empty());
                assert!(filled, "place {p} ({tier:?}) is not a whole bar");
            }
        }
    }

    /// `--watch`'s rectangles are well-formed, clear the screen's borders when they change,
    /// and only then.
    #[test]
    fn rectangles_are_well_formed() {
        let (_, _, borders, _) = drawn(&summary("bars-by-month", 1), EDGED);
        let first = rectangles(&borders, None);
        let (found, _) = commands(&first);
        found.iter().for_each(|d| well_formed(d));
        assert!(found[0].starts_with("1+bcell-border;clear;4095;"), "{:?}", found[0]);
        let again = rectangles(&borders, Some(&borders));
        assert!(!again.contains("cell-border;clear"), "the same borders clear nothing");
        assert!(commands(&again).0.iter().all(|d| d.starts_with("1+bcell-border;set;") || d.starts_with("1+bcell-border;round;")));
    }

    /// The terminal's answers: a border reply before DA1's is support, DA1's alone is none.
    #[test]
    fn the_answers_read() {
        let ghostty = b"\x1bP1+bcell-border;ok;1;pen,rect,half,center,palette,priority,style,round,outside,radius\x1b\\\x1b[?62;22c";
        assert_eq!(Edges::answered(ghostty), Some(Edges { round: true }));
        assert_eq!(Edges::answered(b"\x1bP1+bcell-border;ok;1;pen,rect,half,center,palette\x1b\\\x1b[?1;2c"), Some(Edges { round: false }));
        assert_eq!(Edges::answered(b"\x1b[?62;22c"), None);
        // a reply after DA1's is not an answer to this query
        assert_eq!(Edges::answered(b"\x1b[?62;22c\x1bP1+bcell-border;ok;1;pen,rect\x1b\\"), None);
        assert_eq!(Edges::answered(b"\x1bP1+bcell-border;ok;1;half,center\x1b\\\x1b[?62c"), None);
        // no centre arms, no rules through a row's middle
        assert_eq!(Edges::answered(b"\x1bP1+bcell-border;ok;1;pen,rect,half\x1b\\\x1b[?62c"), None);
        assert!(da1_ends(b"\x1bP1+bcell-border;ok;1;pen\x1b\\\x1b[?62;22c"));
        assert!(!da1_ends(b"\x1bP1+bcell-border;ok;1;pen\x1b\\\x1b[?62;2"));
    }

    /// A card `width` columns wide with `foot`, and its cells.
    fn card(s: &Summary, tier: Tier, width: u16, foot: Foot) -> (Card, Buffer) {
        let area = Rect::new(0, 0, width, 21);
        let mut buf = Buffer::empty(area);
        let card = draw(s, area, &mut buf, &mut Borders::new(area), tier, foot);
        (card, buf)
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (buf.area.left()..buf.area.right()).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// `--watch`'s foot: what was read at its leading end, the keys at its trailing end at
    /// least `APART` cells after it, the keys gone where both do not fit; in both tiers.
    #[test]
    fn the_keys_are_a_segment_of_their_own() {
        let s = summary("bars-by-month", 1);
        let foot = || Foot {
            said: vec![Span::raw(" bars-by-month.cells · read 39s ago · 4 rows ")],
            keys: vec![Span::raw(" "), Span::raw("q"), Span::raw("/"), Span::raw("esc"), Span::raw(" leave ")],
        };
        for tier in [FINE, EDGED] {
            // with edges, a row of room under the foot
            let under = if tier.edges.is_some() { 2 } else { 1 };
            let (c, buf) = card(&s, tier, 80, foot());
            let line = row(&buf, c.height - under);
            let (said, keys) = (line.find("4 rows").expect("what was read") + 6, line.find("q/esc").expect("the keys"));
            assert!(line[said..keys].chars().count() > APART, "{tier:?}: the segments run on: {line:?}");
            assert!(line.trim_end_matches(['╯', '─', ' ']).ends_with("leave"), "{tier:?}: the keys end the foot: {line:?}");
            let (c, buf) = card(&s, tier, 50, foot());
            let line = row(&buf, c.height - under);
            assert!(line.contains("4 rows") && !line.contains("q/esc"), "{tier:?}: at 50 the keys go first: {line:?}");
        }
        // printed and left, no keys
        let (c, buf) = card(&s, FINE, 80, Foot { said: vec![Span::raw(" 4 rows ")], keys: Vec::new() });
        assert!(!row(&buf, c.height - 1).contains("leave"));
    }

    /// A chart of several series names each in its colour, its kind's mark before it, every
    /// name inside the card at any width and in both tiers; one series, or a heat's rows,
    /// none.
    #[test]
    fn every_series_is_named_in_its_colour() {
        let cases = [
            ("stack-modes", 1),
            ("stack-modes", 2),
            ("color-of-text-in-file-order", 1),
            ("from-an-inline-table", 1),
            ("layers-by-name", 3),
            ("color-tinted-and-unworn", 1),
            ("split-of-a-choice-past-the-tints", 1),
        ];
        for (case, n) in cases {
            let s = summary(case, n);
            let parts: Vec<&Summary> = if s.layers.is_empty() { vec![&s] } else { s.layers.iter().collect() };
            for tier in [FINE, EDGED, INDEXED, SIXTEEN] {
                for width in [80, 50, 30] {
                    let (_, buf) = card(&s, tier, width, Foot { said: Vec::new(), keys: Vec::new() });
                    let inner = 2..width - 2;
                    for p in &parts {
                        for (name, shade) in &p.series {
                            let name = if name.is_empty() { p.title.as_str() } else { name.as_str() };
                            let fg = Inks::of(&parts, tier).color(*shade);
                            let mark = swatch(p.kind, tier.glyphs);
                            let shown = (1..10).any(|y| {
                                let line = row(&buf, y);
                                let cut: String = name.chars().take(width as usize - 12).collect();
                                line.find(&cut).is_some_and(|at| {
                                    let x = line[..at].chars().count() as u16;
                                    inner.contains(&(x - 2))
                                        && buf[(x - 2, y)].symbol() == mark
                                        && buf[(x - 2, y)].fg == fg
                                        && buf[(x, y)].fg == fg
                                })
                            });
                            assert!(shown, "{case} {n} at {width} ({tier:?}): {name} is not named in its colour");
                        }
                    }
                }
            }
        }
        for (case, n) in [("bars-by-month", 1), ("line-area-and-steps", 3), ("heat", 1)] {
            for tier in [FINE, EDGED] {
                let (_, buf) = card(&summary(case, n), tier, 80, Foot { said: Vec::new(), keys: Vec::new() });
                for y in 1..4 {
                    let line = row(&buf, y);
                    let first = line.trim_start_matches(['│', ' ']);
                    assert!(
                        !["█ ", "─ ", "┘ ", "• ", "▄ "].iter().any(|m| first.starts_with(m)),
                        "{case} ({tier:?}) has a legend: {line:?}"
                    );
                }
            }
        }
        // a heat's rows named at their left
        let (_, buf) = card(&summary("heat", 1), FINE, 80, Foot { said: Vec::new(), keys: Vec::new() });
        assert!((0..21).any(|y| row(&buf, y).trim_start_matches(['│', ' ']).starts_with("14 ")), "the heat's rows are not named");
    }

    /// Series of different shades are drawn in different colours at every tier: blue and
    /// teal, one cyan by nearness alone in the sixteen, are two. The legend names each in
    /// the same colour (`every_series_is_named_in_its_colour`).
    #[test]
    fn series_stay_apart_at_every_tier() {
        let cases = [
            ("split-of-a-choice-past-the-tints", 1),
            ("split-past-the-tints-folds-the-smallest", 1),
            ("color-tinted-and-unworn", 1),
            ("stack-modes", 1),
            ("layers-by-name", 3),
        ];
        for (case, n) in cases {
            let s = summary(case, n);
            let parts: Vec<&Summary> = if s.layers.is_empty() { vec![&s] } else { s.layers.iter().collect() };
            let shades: Vec<Shade> = parts.iter().flat_map(|p| p.series.iter().map(|x| x.1)).collect();
            for tier in [FINE, INDEXED, SIXTEEN] {
                let inks = Inks::of(&parts, tier);
                let colors: Vec<Color> = shades.iter().map(|sh| inks.color(*sh)).collect();
                for (i, a) in shades.iter().enumerate() {
                    for (j, b) in shades.iter().enumerate() {
                        assert!(a == b || colors[i] != colors[j], "{case} {n} ({tier:?}): {a:?} and {b:?} share {:?}", colors[i]);
                    }
                }
            }
        }
    }
}
