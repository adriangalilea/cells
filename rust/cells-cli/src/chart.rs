//! `cells chart`: a file's charts in a terminal (SPEC.md, Charts). The engine says what each
//! chart draws (`Doc::summary`); here it is drawn into a terminal's cells in what the
//! terminal holds: octants or braille, half blocks and eighths, or plain ASCII; true
//! colour, 256 or 16. Once inline at the terminal's width, or held on the alternate screen
//! and drawn again as the file changes (`--watch`); `--svg` writes the engine's SVG.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cells::chart::{Kind, Shade, Stack, Summary};
use cells_disk::Disk;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::Widget;
use ratatui::widgets::canvas::{Canvas, Line as Segment, Points};

/// What a terminal draws with: its glyphs and its colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub glyphs: Glyphs,
    pub colors: Colors,
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
    /// colours, a console font of blocks and no braille), `COLORTERM` says true colour.
    pub fn guessed() -> Tier {
        let term = std::env::var("TERM").unwrap_or_default();
        let truecolor = matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit"));
        if term == "linux" {
            return Tier { glyphs: Glyphs::Blocks, colors: Colors::Sixteen };
        }
        if term == "dumb" {
            return Tier { glyphs: Glyphs::Ascii, colors: Colors::Sixteen };
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
        }
    }
}

/// What `cells chart` was asked.
struct Asked {
    path: PathBuf,
    charts: Vec<usize>,
    watch: bool,
    svg: bool,
    tier: Tier,
}

/// `cells chart <file> [n …] [--watch] [--svg] [--glyphs fine|blocks|ascii]
/// [--colors true|256|16]`. No numbers lists the file's charts.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut asked = Asked { path: PathBuf::new(), charts: Vec::new(), watch: false, svg: false, tier: Tier::guessed() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--watch" => asked.watch = true,
            "--svg" => asked.svg = true,
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
    if asked.watch {
        return watch(&asked, &summaries);
    }
    let width = terminal_width();
    let height = if io::stdout().is_terminal() { 16 } else { 12 };
    let mut out = io::stdout().lock();
    for s in &drawn {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        draw(s, area, &mut buf, asked.tier);
        print(&buf, asked.tier, &mut out).map_err(|e| e.to_string())?;
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
/// terminal is resized; `q` or Esc leaves.
fn watch(asked: &Asked, summaries: &dyn Fn(&Path) -> Result<(Vec<(String, Kind)>, Vec<Summary>), String>) -> Result<(), String> {
    use ratatui::crossterm::event::{self, Event, KeyCode};
    let mut terminal = ratatui::init();
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut seen = modified(&asked.path);
    let mut drawn = summaries(&asked.path).map(|(_, d)| d);
    let result = loop {
        let frame = terminal.draw(|f| {
            let area = f.area();
            match &drawn {
                Ok(charts) => {
                    let n = charts.len().max(1) as u16;
                    let h = area.height / n;
                    for (i, s) in charts.iter().enumerate() {
                        let part = Rect::new(area.x, area.y + h * i as u16, area.width, h);
                        draw(s, part, f.buffer_mut(), asked.tier);
                    }
                }
                Err(why) => {
                    f.buffer_mut().set_string(area.x, area.y, why, Style::default().fg(Color::Red));
                }
            }
        });
        if let Err(e) = frame {
            break Err(e.to_string());
        }
        if event::poll(Duration::from_millis(400)).map_err(|e| e.to_string())? {
            match event::read().map_err(|e| e.to_string())? {
                Event::Key(k) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => break Ok(()),
                _ => {}
            }
        }
        let now = modified(&asked.path);
        if now != seen {
            seen = now;
            drawn = summaries(&asked.path).map(|(_, d)| d);
        }
    };
    ratatui::restore();
    result
}

/// A summary drawn into `area`: its title, its legend, its scale and its marks.
pub fn draw(s: &Summary, area: Rect, buf: &mut Buffer, tier: Tier) {
    if area.height < 4 || area.width < 16 {
        return;
    }
    buf.set_string(area.x, area.y, &s.title, Style::default().add_modifier(Modifier::BOLD));
    let parts: Vec<&Summary> = if s.layers.is_empty() { vec![s] } else { s.layers.iter().collect() };
    // the legend, once there is more than one series
    let series: Vec<(String, Shade)> =
        parts.iter().flat_map(|p| p.series.iter().map(|(n, sh)| (if n.is_empty() { p.title.clone() } else { n.clone() }, *sh))).collect();
    let mut y0 = area.y + 1;
    if series.len() > 1 {
        let mut x = area.x;
        for (name, shade) in &series {
            let mark = if tier.glyphs == Glyphs::Ascii { "* " } else { "● " };
            buf.set_string(x, y0, mark, Style::default().fg(color(*shade, tier)));
            buf.set_string(x + 2, y0, name, Style::default());
            x += name.chars().count() as u16 + 4;
            if x >= area.right() {
                break;
            }
        }
        y0 += 1;
    }
    let body = Rect::new(area.x, y0, area.width, area.bottom().saturating_sub(y0));
    if s.kind == Kind::Figure && s.layers.is_empty() {
        figure(s, body, buf, tier);
        return;
    }
    let num = |v: &Option<String>| v.as_deref().and_then(|t| t.parse::<f64>().ok());
    // the scale: from zero (or below it) to the most, stacked where bars stack
    let stacked = |p: &Summary| p.kind == Kind::Bars && p.stack == Stack::Stacked;
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for p in &parts {
        for row in &p.values {
            let vals: Vec<f64> = row.iter().filter_map(num).collect();
            if stacked(p) {
                hi = hi.max(vals.iter().filter(|v| **v > 0.0).sum());
                lo = lo.min(vals.iter().filter(|v| **v < 0.0).sum());
            } else {
                for v in &vals {
                    hi = hi.max(*v);
                    lo = lo.min(*v);
                }
            }
        }
    }
    if hi <= lo {
        hi = lo + 1.0;
    }
    let labels = [s.figure(hi), s.figure(lo)];
    let left = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16 + 1;
    let plot = Rect::new(body.x + left, body.y, body.width.saturating_sub(left), body.height.saturating_sub(1));
    if plot.width < 4 || plot.height < 2 {
        return;
    }
    let quiet = Style::default().fg(Color::DarkGray);
    buf.set_string(body.x, plot.y, &labels[0], quiet);
    buf.set_string(body.x, plot.bottom() - 1, &labels[1], quiet);
    // the places under the plot, as many as fit
    let places = parts[0].places.len().max(1);
    let band = plot.width as f64 / places as f64;
    let mut free = plot.x;
    for (i, name) in parts[0].places.iter().enumerate() {
        let at = plot.x + (band * i as f64) as u16;
        if at >= free && at + name.chars().count() as u16 <= plot.right() {
            buf.set_string(at, plot.bottom(), name, quiet);
            free = at + name.chars().count() as u16 + 1;
        }
    }
    for part in &parts {
        match part.kind {
            Kind::Bars => bars(part, plot, buf, tier, (lo, hi), stacked(part)),
            Kind::Heat => heat(part, plot, buf, tier),
            _ => lines(part, plot, buf, tier, (lo, hi)),
        }
    }
}

/// Bars in eighths along the value: where two stacked series meet in one cell, the lower
/// is the glyph's ink and the upper its ground, so a stack has no gap.
fn bars(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier, (lo, hi): (f64, f64), stacked: bool) {
    let eighths = ["", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let rows = plot.height as f64;
    let units = |v: f64| ((v - lo) / (hi - lo) * rows * 8.0).round() as i64;
    let zero = units(0.0);
    let places = s.places.len().max(1);
    let band = plot.width as f64 / places as f64;
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
        let width = ((band * 0.7).max(1.0)) as u16;
        let x0 = plot.x + (band * p as f64 + (band - width as f64) / 2.0) as u16;
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
                    // a span that starts above the cell's foot (a bar below zero's top): its
                    // upper part, drawn as the ground over a lower eighth left empty
                    (eighths[lower.0 as usize], usize::MAX, Some(lower.2))
                };
                for dx in 0..w {
                    if let Some(cell) = buf.cell_mut((x + dx, plot.y + r)) {
                        cell.set_symbol(glyph);
                        if fg != usize::MAX {
                            cell.set_fg(color(s.series[fg].1, tier));
                        }
                        if let Some(b) = bg {
                            cell.set_bg(color(s.series[b].1, tier));
                        }
                    }
                }
            }
        }
    }
}

/// Lines, areas, steps and points on a canvas of the tier's finest marks.
fn lines(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier, (lo, hi): (f64, f64)) {
    let marker = match tier.glyphs {
        Glyphs::Fine => Marker::Braille,
        Glyphs::Blocks => Marker::HalfBlock,
        Glyphs::Ascii => Marker::Dot,
    };
    let n = s.places.len().max(2) as f64;
    Canvas::default()
        .marker(marker)
        .x_bounds([-0.5, n - 0.5])
        .y_bounds([lo, hi])
        .paint(|ctx| {
            for (k, (_, shade)) in s.series.iter().enumerate() {
                let c = color(*shade, tier);
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
                                    ctx.draw(&Segment { x1: x, y1: 0.0f64.max(lo), x2: x, y2: y, color: c });
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

/// A cell a pair of places, its depth the value: shades of one tint.
fn heat(s: &Summary, plot: Rect, buf: &mut Buffer, tier: Tier) {
    let all: Vec<f64> = s.values.iter().flatten().filter_map(|v| v.as_deref().and_then(|t| t.parse::<f64>().ok())).collect();
    let (lo, hi) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let tint = s.series.first().map_or(Shade::Quiet, |x| x.1);
    let cols = s.places.len().max(1) as u16;
    let rows = s.series.len().max(1) as u16;
    let (cw, ch) = ((plot.width / cols).max(1), (plot.height / rows).max(1));
    let shades = if tier.glyphs == Glyphs::Ascii { [".", ":", "*", "#"] } else { ["░", "▒", "▓", "█"] };
    for (p, row) in s.values.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            let Some(v) = v.as_deref().and_then(|t| t.parse::<f64>().ok()) else { continue };
            let depth = if hi > lo { (v - lo) / (hi - lo) } else { 1.0 };
            let glyph = shades[((depth * 3.0).round() as usize).min(3)];
            for dy in 0..ch {
                for dx in 0..cw {
                    if let Some(cell) = buf.cell_mut((plot.x + p as u16 * cw + dx, plot.y + k as u16 * ch + dy)) {
                        cell.set_symbol(glyph).set_fg(color(tint, tier));
                    }
                }
            }
        }
    }
}

/// One number, its change from the place before, and its run as a sparkline.
fn figure(s: &Summary, area: Rect, buf: &mut Buffer, tier: Tier) {
    let run: Vec<f64> = s.values.iter().filter_map(|r| r.first().cloned().flatten().and_then(|t| t.parse::<f64>().ok())).collect();
    let Some(&now) = run.last() else { return };
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
        let tint = color(s.series.first().map_or(Shade::Quiet, |x| x.1), tier);
        for (i, v) in run.iter().rev().take(area.width as usize).rev().enumerate() {
            let level = if hi > lo { ((v - lo) / (hi - lo) * 7.0).round() as usize } else { 7 };
            buf.set_string(area.x + i as u16, area.y + 2, bars[level], Style::default().fg(tint));
        }
    }
}

/// A shade as the terminal draws it: true colour, the 256's nearest, or the sixteen's.
fn color(shade: Shade, tier: Tier) -> Color {
    let hex = shade.hex(true);
    let rgb = u32::from_str_radix(&hex[1..], 16).expect("a shade is #rrggbb");
    let (r, g, b) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    match tier.colors {
        Colors::True => Color::Rgb(r, g, b),
        Colors::Indexed => {
            let step = |c: u8| ((c as u16 * 5 + 127) / 255) as u8;
            Color::Indexed(16 + 36 * step(r) + 6 * step(g) + step(b))
        }
        Colors::Sixteen => {
            // the sixteen by their usual values, the nearest taken
            const ANSI: [(Color, (u8, u8, u8)); 12] = [
                (Color::Red, (205, 49, 49)),
                (Color::Green, (13, 188, 121)),
                (Color::Yellow, (229, 229, 16)),
                (Color::Blue, (36, 114, 200)),
                (Color::Magenta, (188, 63, 188)),
                (Color::Cyan, (17, 168, 205)),
                (Color::Gray, (229, 229, 229)),
                (Color::DarkGray, (102, 102, 102)),
                (Color::LightRed, (241, 76, 76)),
                (Color::LightGreen, (35, 209, 139)),
                (Color::LightYellow, (245, 245, 67)),
                (Color::LightMagenta, (214, 112, 214)),
            ];
            let d = |(x, y, z): (u8, u8, u8)| {
                let (dr, dg, db) = (x as i32 - r as i32, y as i32 - g as i32, z as i32 - b as i32);
                dr * dr + dg * dg + db * db
            };
            ANSI.iter().min_by_key(|(_, c)| d(*c)).map(|(c, _)| *c).unwrap_or(Color::Gray)
        }
    }
}

/// A buffer printed inline: each row its cells with their colours, the colours reset at
/// its end.
fn print(buf: &Buffer, tier: Tier, out: &mut impl Write) -> io::Result<()> {
    let sgr = |c: Color, ground: bool| -> Option<String> {
        let base = if ground { 40 } else { 30 };
        match c {
            Color::Reset => None,
            Color::Rgb(r, g, b) => Some(format!("{};2;{r};{g};{b}", base + 8)),
            Color::Indexed(i) => Some(format!("{};5;{i}", base + 8)),
            named => {
                let (n, bright) = match named {
                    Color::Black => (0, false),
                    Color::Red => (1, false),
                    Color::Green => (2, false),
                    Color::Yellow => (3, false),
                    Color::Blue => (4, false),
                    Color::Magenta => (5, false),
                    Color::Cyan => (6, false),
                    Color::Gray => (7, false),
                    Color::DarkGray => (0, true),
                    Color::LightRed => (1, true),
                    Color::LightGreen => (2, true),
                    Color::LightYellow => (3, true),
                    Color::LightBlue => (4, true),
                    Color::LightMagenta => (5, true),
                    Color::LightCyan => (6, true),
                    _ => (7, true),
                };
                Some(format!("{}", if bright { base + 60 + n } else { base + n }))
            }
        }
    };
    let colored = io::stdout().is_terminal() || tier.colors != Colors::Sixteen;
    let area = buf.area;
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            let cell = &buf[(x, y)];
            let codes: Vec<String> = [sgr(cell.fg, false), sgr(cell.bg, true)].into_iter().flatten().collect();
            let bold = cell.modifier.contains(Modifier::BOLD);
            if colored && (!codes.is_empty() || bold) {
                line += &format!("\x1b[{}{}m{}\x1b[0m", if bold { "1;" } else { "" }, codes.join(";"), cell.symbol());
            } else {
                line += cell.symbol();
            }
        }
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}
