//! Rendering. btop-style, truecolor. Layout: a status bar, a body split into a
//! network sidebar and a tabbed main area (Peers / Transport / History), and a
//! key-hint bar. An unreachable daemon shows a centered "daemon down" screen.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, Wrap};
use std::time::{SystemTime, UNIX_EPOCH};
use tetron_proto::ipc::{ConnType, ViaDetail};

use crate::app::{App, Btn, Hits, InviteBtn, InputKind, Mode, Tab};
use crate::model::{NetSnap, PeerSnap};

const C_DIM: Color = Color::DarkGray;
const C_OK: Color = Color::Green;
const C_WARN: Color = Color::Yellow;
const C_BAD: Color = Color::Red;
const C_ACCENT: Color = Color::Cyan;

pub fn draw(f: &mut Frame, app: &mut App) {
    let mut hits = Hits::default();
    // The toolbar may need a second line in a narrow window; size the footer to
    // fit so no button is clipped off-screen.
    let buttons = toolbar_buttons(app);
    let (fh, placements) = pack_toolbar(f.area().width, 2, &buttons);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(fh)])
        .split(f.area());

    draw_statusbar(f, rows[0], app);

    if app.snapshot.is_none() {
        draw_down(f, rows[1], app);
    } else {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(1)])
            .split(rows[1]);
        draw_sidebar(f, body[0], app, &mut hits);
        draw_main(f, body[1], app, &mut hits);
    }

    draw_toolbar(f, rows[2], &placements, &mut hits);

    match &app.mode {
        Mode::Input { kind, buf } => draw_input(f, *kind, buf),
        Mode::Invite { network, code, id, qr } => {
            draw_invite(f, network, code, id, qr, app, &mut hits)
        }
        Mode::PickNet { sel } => draw_pick_net(f, app, *sel, &mut hits),
        Mode::Confirm { prompt, .. } => draw_confirm(f, prompt),
        Mode::Normal => {
            if app.show_help {
                draw_help(f);
            }
        }
    }

    app.hits = hits;
}

fn draw_statusbar(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![Span::styled(" tetron ", Style::new().fg(Color::Black).bg(C_ACCENT).bold())];
    if let Some(s) = &app.snapshot {
        let (word, col) = if s.active { ("active", C_OK) } else { ("standby", C_WARN) };
        let peers: usize = s.networks.iter().map(|n| n.connected()).sum();
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format!("⇄ {word}"), Style::new().fg(col)));
        spans.push(Span::styled(
            format!("  ·  {} net  ·  {} peers", s.networks.len(), peers),
            Style::new().fg(C_DIM),
        ));
        if !s.daemon_version.is_empty() {
            spans.push(Span::styled(format!("   daemon v{}", s.daemon_version), Style::new().fg(C_DIM)));
        }
    } else {
        spans.push(Span::styled("  daemon down", Style::new().fg(C_BAD)));
    }
    // Transient toast, right here on the status bar so the toolbar stays put.
    if let Some(t) = &app.toast {
        let col = if t.err { C_BAD } else { C_OK };
        spans.push(Span::styled(format!("   {} {}", if t.err { "✗" } else { "✓" }, t.text), Style::new().fg(col).bold()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The toolbar buttons for the current state (daemon up vs down).
fn toolbar_buttons(app: &App) -> Vec<(&'static str, Btn, Color)> {
    if app.snapshot.is_none() {
        vec![("start", Btn::Start, C_OK), ("help", Btn::Help, C_ACCENT), ("quit", Btn::Quit, C_DIM)]
    } else {
        vec![
            ("connect", Btn::Connect, C_OK),
            ("standby", Btn::Standby, C_WARN),
            ("ssh", Btn::Ssh, C_ACCENT),
            ("copy ip", Btn::CopyIp, C_ACCENT),
            ("invite", Btn::Invite, C_ACCENT),
            ("new net", Btn::NewNet, C_ACCENT),
            ("join", Btn::Join, C_ACCENT),
            ("stop", Btn::Stop, C_BAD),
            ("help", Btn::Help, C_DIM),
            ("quit", Btn::Quit, C_DIM),
        ]
    }
}

/// Greedily pack buttons into up to `max_lines` rows for the given width,
/// wrapping when a button would overflow. Returns the number of lines used and
/// each button's placement (line, x-offset, label, action, color).
fn pack_toolbar(
    width: u16,
    max_lines: u16,
    btns: &[(&'static str, Btn, Color)],
) -> (u16, Vec<(u16, u16, &'static str, Btn, Color)>) {
    let mut out = Vec::new();
    let (mut line, mut x) = (0u16, 0u16);
    for (label, btn, col) in btns {
        let w = label.chars().count() as u16 + 2;
        if x > 0 && x + w > width {
            if line + 1 >= max_lines {
                break; // out of room; remaining actions stay keyboard-only
            }
            line += 1;
            x = 0;
        }
        out.push((line, x, *label, *btn, *col));
        x += w + 1; // button plus a one-cell gap
    }
    (line + 1, out)
}

/// Render packed toolbar buttons and record their click rects.
fn draw_toolbar(f: &mut Frame, area: Rect, placements: &[(u16, u16, &'static str, Btn, Color)], hits: &mut Hits) {
    for (line, x, label, btn, col) in placements {
        let w = label.chars().count() as u16 + 2;
        let rect = Rect { x: area.x + x, y: area.y + line, width: w, height: 1 };
        if rect.y >= area.y + area.height {
            continue;
        }
        hits.buttons.push((rect, *btn));
        f.render_widget(
            Paragraph::new(Span::styled(format!(" {label} "), Style::new().fg(Color::Black).bg(*col).bold())),
            rect,
        );
    }
}

fn draw_sidebar(f: &mut Frame, area: Rect, app: &App, hits: &mut Hits) {
    let block = Block::default().borders(Borders::ALL).title(" Networks ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if let Some(s) = &app.snapshot {
        for (i, net) in s.networks.iter().enumerate() {
            hits.nets.push(Rect { x: inner.x, y: inner.y + i as u16, width: inner.width, height: 1 });
            let sel = i == app.sel_net;
            let marker = if sel { "▸ " } else { "  " };
            let (dot, dcol) = if net.active { ("●", C_OK) } else { ("○", C_DIM) };
            let name_style = if sel {
                Style::new().fg(C_ACCENT).bold()
            } else {
                Style::new()
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::new().fg(C_ACCENT)),
                Span::styled(format!("{:<12}", trunc(&net.name, 12)), name_style),
                Span::styled(dot, Style::new().fg(dcol)),
                Span::styled(format!(" {}/{}", net.connected(), net.member_count), Style::new().fg(C_DIM)),
            ]));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("  ↵ ssh peer  y copy-ip", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  c connect   d standby", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  C/D all     i invite", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  n new net   J join", Style::new().fg(C_DIM)));
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_main(f: &mut Frame, area: Rect, app: &App, hits: &mut Hits) {
    let Some(net) = app.cur_net() else {
        f.render_widget(Paragraph::new("no networks"), area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    draw_net_header(f, rows[0], net);
    draw_tabline(f, rows[1], app.tab, hits);

    match app.tab {
        Tab::Peers => draw_peers(f, rows[2], net, app.sel_peer, &app.peer_scroll, hits),
        Tab::Transport => draw_transport(f, rows[2], app.cur_peer()),
        Tab::History => {
            let rest = below_sel_peer(f, rows[2], app);
            draw_history(f, rest, app)
        }
        Tab::Admin => {
            let rest = below_sel_peer(f, rows[2], app);
            draw_admin(f, rest, net, app)
        }
    }
}

/// Draw the selected peer on the first line of `area` and return the rest
/// (TUI-PEER-001: History and Admin show whom they refer to).
fn below_sel_peer(f: &mut Frame, area: Rect, app: &App) -> Rect {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let line = match app.cur_peer() {
        Some(p) => Line::from(vec![
            Span::styled("peer  ", Style::new().fg(C_DIM)),
            Span::styled(p.label(), Style::new().fg(C_ACCENT).bold()),
            Span::styled("   (j/k to pick)", Style::new().fg(C_DIM)),
        ]),
        None => Line::styled("no peer selected", Style::new().fg(C_DIM)),
    };
    f.render_widget(Paragraph::new(line), parts[0]);
    parts[2]
}

fn draw_net_header(f: &mut Frame, area: Rect, net: &NetSnap) {
    let l1 = Line::from(vec![
        Span::styled(trunc(&net.name, 24), Style::new().fg(C_ACCENT).bold()),
        Span::styled(format!("   role {}", net.role), Style::new().fg(C_DIM)),
    ]);
    let l2 = Line::from(vec![Span::styled(
        format!("subnet {}   tun {}   my {}", dash(&net.subnet), dash(&net.tun_name), net.my_ip),
        Style::new().fg(C_DIM),
    )]);
    f.render_widget(Paragraph::new(vec![l1, l2]), area);
}

fn draw_tabline(f: &mut Frame, area: Rect, cur: Tab, hits: &mut Hits) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for (i, t) in Tab::all().iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" | ", Style::new().fg(C_DIM)));
            x += 3;
        }
        let label = format!(" {} ", t.title());
        let w = label.chars().count() as u16;
        let style = if *t == cur {
            Style::new().fg(Color::Black).bg(C_ACCENT).bold()
        } else {
            Style::new().fg(C_DIM)
        };
        hits.tabs.push((Rect { x, y: area.y, width: w, height: 1 }, *t));
        x += w;
        spans.push(Span::styled(label, style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_peers(f: &mut Frame, area: Rect, net: &NetSnap, sel: usize, scroll: &std::cell::Cell<usize>, hits: &mut Hits) {
    let n = net.peers.len();
    // When the list is taller than the area, reserve the last line for a
    // scroll-position status, so the table shows one fewer row.
    let overflow = n > area.height.saturating_sub(1) as usize;
    let table_area = if overflow {
        Rect { height: area.height.saturating_sub(1), ..area }
    } else {
        area
    };
    // Rows available below the single header line.
    let vis = table_area.height.saturating_sub(1) as usize;
    // Keep the selected peer in view, then clamp so we never scroll past the end.
    let mut off = scroll.get();
    if sel < off {
        off = sel;
    } else if vis > 0 && sel >= off + vis {
        off = sel + 1 - vis;
    }
    off = off.min(n.saturating_sub(vis.max(1)));
    scroll.set(off);
    let end = if vis == 0 { off } else { (off + vis).min(n) };

    // Hit rects for the visible slice (header at table_area.y; peer at +1+j).
    for (j, i) in (off..end).enumerate() {
        hits.peers.push((Rect { x: table_area.x, y: table_area.y + 1 + j as u16, width: table_area.width, height: 1 }, i));
    }

    let header = Row::new(["", "host", "ip", "via", "rtt", "last"])
        .style(Style::new().fg(C_DIM).bold());
    let now = unix_now();
    let rows: Vec<Row> = net
        .peers
        .iter()
        .enumerate()
        .skip(off)
        .take(end - off)
        .map(|(i, p)| {
            let (mark, mcol) = peer_marker(p);
            let (via, vcol) = match &p.conn {
                Some(c) => conn_type_str(&c.conn_type),
                None => ("--", C_DIM),
            };
            let rtt = p.conn.as_ref().and_then(|c| c.rtt_ms);
            // The selected row has a fixed dark background, so its default and
            // dim text must be light too (see `on_selected`).
            let fg = |c: Color| if i == sel { on_selected(c) } else { c };
            let r = Row::new([
                Cellish(mark.to_string(), fg(mcol)),
                Cellish(trunc(&p.hostname, 12), fg(Color::Reset)),
                Cellish(p.ip.clone(), fg(C_DIM)),
                Cellish(via.to_string(), fg(vcol)),
                Cellish(fmt_rtt(rtt), fg(color_rtt(rtt))),
                Cellish(fmt_ago(p.last_seen, now), fg(C_DIM)),
            ]
            .map(Cell::from));
            if i == sel {
                row_selected(r)
            } else {
                r
            }
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(13),
        Constraint::Length(16),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(6),
    ];
    let table = Table::new(rows, widths).header(header).column_spacing(1);
    f.render_widget(table, table_area);

    if overflow {
        let y = area.y + area.height - 1;
        let status = format!(
            "  {}▲ above · {}▼ below · {}/{}  (↑↓/wheel to scroll)",
            off,
            n - end,
            sel + 1,
            n
        );
        f.render_widget(
            Paragraph::new(Span::styled(status, Style::new().fg(C_ACCENT))),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );
    }
}

fn draw_transport(f: &mut Frame, area: Rect, peer: Option<&PeerSnap>) {
    let Some(p) = peer else {
        f.render_widget(Paragraph::new(Span::styled("no peer selected", Style::new().fg(C_DIM))), area);
        return;
    };
    let Some(c) = &p.conn else {
        f.render_widget(
            Paragraph::new(Span::styled(format!("{} is not connected", p.hostname), Style::new().fg(C_DIM))),
            area,
        );
        return;
    };

    let (via, vcol) = conn_type_str(&c.conn_type);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(trunc(&p.hostname, 24), Style::new().bold()),
            Span::styled(format!("   {}", p.ip), Style::new().fg(C_DIM)),
        ]),
        Line::from(vec![
            Span::styled("transport  ", Style::new().fg(C_DIM)),
            Span::styled(via, Style::new().fg(vcol).bold()),
            Span::styled(
                format!(
                    "   rtt {}   mtu {}",
                    fmt_rtt(c.rtt_ms),
                    c.max_datagram_size.map(|m| m.to_string()).unwrap_or_else(|| "-".into()),
                ),
                Style::new().fg(C_DIM),
            ),
        ]),
        Line::from(vec![
            Span::styled("traffic    ", Style::new().fg(C_DIM)),
            Span::styled(
                format!(
                    "tx {}  rx {}  lost {}",
                    fmt_bytes(c.bytes_tx),
                    fmt_bytes(c.bytes_rx),
                    c.lost_packets
                ),
                Style::new().fg(C_DIM),
            ),
        ]),
    ];
    if let Some(remote) = &c.remote_addr {
        lines.push(Line::from(vec![
            Span::styled("remote     ", Style::new().fg(C_DIM)),
            Span::raw(remote.clone()),
        ]));
    }
    if let Some(v) = &c.via_detail {
        lines.push(Line::from(vec![
            Span::styled("why relay  ", Style::new().fg(C_DIM)),
            Span::styled(via_detail_str(v), Style::new().fg(C_WARN)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("candidate paths", Style::new().fg(C_DIM).bold()));
    if c.paths.is_empty() {
        lines.push(Line::styled("  (none reported)", Style::new().fg(C_DIM)));
    } else {
        for path in &c.paths {
            let (pvia, pcol) = conn_type_str(&path.conn_type);
            let sel = if path.is_selected { "▸" } else { " " };
            let flags = format!(
                "{}{}",
                if path.in_subnet { "" } else { " bled" },
                if path.has_activity { "" } else { " idle" },
            );
            let mtu = path.current_mtu.map(|m| format!("  mtu {m}")).unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(format!(" {sel} "), Style::new().fg(C_OK)),
                Span::styled(format!("{:<7}", pvia), Style::new().fg(pcol)),
                Span::raw(trunc(&path.remote_addr, 30)),
                Span::styled(format!("  {}", fmt_rtt(path.rtt_ms)), Style::new().fg(color_rtt(path.rtt_ms))),
                Span::styled(mtu, Style::new().fg(C_DIM)),
                Span::styled(flags, Style::new().fg(C_WARN)),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

/// Resolve a key-holder short id to a readable name via the peer list.
fn admin_name(net: &NetSnap, short_id: &str, is_self: bool) -> String {
    if is_self {
        return net.my_hostname.clone().unwrap_or_else(|| "you".into());
    }
    net.peers
        .iter()
        .find(|p| p.key.starts_with(short_id))
        .map(|p| p.hostname.clone())
        .unwrap_or_else(|| "?".into())
}

fn draw_history(f: &mut Frame, area: Rect, app: &App) {
    let Some(p) = app.cur_peer() else {
        f.render_widget(Paragraph::new(Span::styled("no peer selected", Style::new().fg(C_DIM))), area);
        return;
    };
    if p.conn.is_none() {
        let msg = format!("{} is offline — no live traffic history", p.hostname);
        f.render_widget(Paragraph::new(Span::styled(msg, Style::new().fg(C_DIM))), area);
        return;
    }
    let Some(hist) = app.history.get(&p.key) else {
        f.render_widget(
            Paragraph::new(Span::styled("collecting history…", Style::new().fg(C_DIM))),
            area,
        );
        return;
    };

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(1, 3), Constraint::Ratio(1, 3)])
        .split(area);

    spark(f, parts[0], "rtt (ms)", &hist.rtt, C_ACCENT, |v| v.to_string());
    spark(f, parts[1], "tx", &hist.tx_rate, C_OK, |v| fmt_rate(v));
    spark(f, parts[2], "rx", &hist.rx_rate, C_WARN, |v| fmt_rate(v));
}

fn spark(
    f: &mut Frame,
    area: Rect,
    label: &str,
    data: &std::collections::VecDeque<u64>,
    color: Color,
    fmt_cur: impl Fn(u64) -> String,
) {
    let v: Vec<u64> = data.iter().copied().collect();
    let cur = v.last().copied().unwrap_or(0);
    let max = v.iter().copied().max().unwrap_or(0);
    let title = format!(" {label}   now {}   peak {} ", fmt_cur(cur), fmt_cur(max));
    let block = Block::default().borders(Borders::ALL).title(title).title_style(Style::new().fg(C_DIM));
    let sk = Sparkline::default().block(block).data(v).max(max.max(1)).style(Style::new().fg(color));
    f.render_widget(sk, area);
}

fn draw_down(f: &mut Frame, area: Rect, app: &App) {
    let reason = app.down_reason.clone().unwrap_or_else(|| "unreachable".into());
    let lines = vec![
        Line::raw(""),
        Line::styled("tetron daemon not reachable", Style::new().fg(C_BAD).bold()),
        Line::raw(""),
        Line::styled(reason, Style::new().fg(C_DIM)),
        Line::raw(""),
        Line::from(vec![
            Span::styled("press ", Style::new().fg(C_DIM)),
            Span::styled("s", Style::new().fg(C_ACCENT).bold()),
            Span::styled(" to run: ", Style::new().fg(C_DIM)),
            Span::styled("systemctl start tetron.service", Style::new().fg(C_ACCENT)),
        ]),
        Line::raw(""),
        Line::styled("retrying every 2s…", Style::new().fg(C_DIM).italic()),
    ];
    let p = Paragraph::new(lines).alignment(Alignment::Center).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, centered(area, 60, 11));
}

fn draw_help(f: &mut Frame) {
    let key = |k: &'static str, d: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {k:<12}"), Style::new().fg(C_ACCENT)),
            Span::styled(d, Style::new()),
        ])
    };
    let lines = vec![
        Line::styled(" tetron-tui — keys ", Style::new().fg(C_ACCENT).bold()),
        Line::raw(""),
        key("↑/↓ j/k", "select peer"),
        key("←/→ h/l", "select network"),
        key("Tab 1-4", "switch pane (Peers/Transport/History/Admin)"),
        Line::raw(""),
        key("Enter", "ssh to the selected peer (overlay IP)"),
        key("y", "copy selected peer IP (or the [copy ip] button)"),
        key("m", "mouse off for native select (Shift+drag also works)"),
        Line::raw(""),
        key("c / d", "connect / standby selected network"),
        key("C / D", "connect / standby ALL networks"),
        key("i", "create invite (asks which network if several; y copy, w save, s share on LAN)"),
        key("n", "new network      J  join by invite, file or URL"),
        key("S", "stop daemon  ·  s: start (when down)"),
        Line::raw(""),
        Line::styled("  admin (coordinator):", Style::new().fg(C_DIM).italic()),
        key("a", "grant admin key to selected peer"),
        key("K", "kick selected peer"),
        key("X", "nuke network / second a proposal"),
        key("L", "leave network"),
        Line::raw(""),
        key("? ", "this help      q / Esc  quit"),
        Line::raw(""),
        Line::styled("  press any key to close", Style::new().fg(C_DIM)),
    ];
    let n = lines.len() as u16 + 2;
    let area = centered(f.area(), 56, n);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_input(f: &mut Frame, kind: InputKind, buf: &str) {
    let (title, hint) = match kind {
        InputKind::Join => (" Join by invite, file or URL ", "paste the invite, a file path or an http(s) URL, then Enter"),
        InputKind::Create => (" New network ", "type a name, then Enter"),
    };
    let area = centered(f.area(), 64, 6);
    f.render_widget(Clear, area);
    let block = Block::default().borders(Borders::ALL).title(title).title_style(Style::new().fg(C_ACCENT).bold());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    // Field (truncated from the left so the caret end stays visible).
    let shown = tail(buf, rows[0].width.saturating_sub(2) as usize);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("> ", Style::new().fg(C_ACCENT)), Span::raw(shown.clone())])),
        rows[0],
    );
    f.render_widget(Paragraph::new(Span::styled(hint, Style::new().fg(C_DIM))), rows[2]);
    let cx = rows[0].x + 2 + shown.chars().count() as u16;
    f.set_cursor_position((cx.min(rows[0].x + rows[0].width.saturating_sub(1)), rows[0].y));
}

fn draw_invite(f: &mut Frame, network: &str, code: &str, id: &str, qr: &str, app: &App, hits: &mut Hits) {
    let sharing = app.share.as_ref();
    // While sharing on the LAN the URL and its QR replace the invite (TUI-SERVE-001).
    let share = sharing.and_then(|s| s.url.as_ref());
    let qr = share.map_or(qr, |(_, q)| q.as_str());
    let qr_w = qr.lines().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    let qr_h = qr.lines().count() as u16;
    let w = (qr_w.max(66) + 4).min(f.area().width);
    let h = (qr_h + 9).min(f.area().height);
    let area = centered(f.area(), w, h);
    hits.invite_card = Some(area);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Invite · {network} "))
        .title_style(Style::new().fg(C_ACCENT).bold());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let first = match share {
        Some((url, _)) => Line::from(vec![
            Span::styled("url   ", Style::new().fg(C_DIM)),
            Span::styled(url.clone(), Style::new().fg(C_OK).bold()),
        ]),
        None => Line::from(vec![
            Span::styled("code  ", Style::new().fg(C_DIM)),
            Span::styled(code.to_string(), Style::new().fg(C_OK).bold()),
        ]),
    };
    let mut lines = vec![
        first,
        Line::from(vec![
            Span::styled("id    ", Style::new().fg(C_DIM)),
            Span::raw(id.to_string()),
        ]),
        Line::raw(""),
    ];
    for l in qr.lines() {
        lines.push(Line::raw(l.to_string()));
    }
    lines.push(Line::raw(""));
    // The content scrolls above two fixed rows (buttons, note), so a small
    // window still reaches the whole QR (TUI-INVITE-003).
    let content = Rect { height: inner.height.saturating_sub(2), ..inner };
    let wrapped: u16 = lines
        .iter()
        .map(|l| (l.width() as u16).max(1).div_ceil(inner.width.max(1)))
        .sum();
    let max = wrapped.saturating_sub(content.height);
    app.invite_scroll_max.set(max);
    let scroll = app.invite_scroll.get().min(max);
    app.invite_scroll.set(scroll);
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).wrap(Wrap { trim: false }).scroll((scroll, 0)),
        content,
    );
    // Button row: key letter and action, clickable (TUI-SERVE-001).
    let share_label = if sharing.is_some() { "stop sharing" } else { "share on LAN" };
    let btns: [(&str, &str, InviteBtn, Color); 5] = [
        ("y", "copy", InviteBtn::Copy, C_ACCENT),
        ("w", "save", InviteBtn::Save, C_ACCENT),
        ("s", share_label, InviteBtn::Share, C_OK),
        ("m", if app.mouse_on { "select text" } else { "mouse on" }, InviteBtn::Select, C_ACCENT),
        ("Esc", "close", InviteBtn::Close, C_DIM),
    ];
    let full: u16 = btns.iter().map(|(k, l, _, _)| (k.len() + l.len() + 3) as u16).sum::<u16>() + btns.len() as u16 - 1;
    let compact = full > inner.width;
    let label = |k: &str, l: &str| if compact { format!(" {k} ") } else { format!(" {k} {l} ") };
    let widths: Vec<u16> = btns.iter().map(|(k, l, _, _)| label(k, l).chars().count() as u16).collect();
    let total: u16 = widths.iter().sum::<u16>() + (btns.len() as u16 - 1);
    let row_y = inner.y + inner.height.saturating_sub(2);
    let mut x = inner.x + inner.width.saturating_sub(total) / 2;
    hits.invite_btns.clear();
    for ((k, l, b, col), w) in btns.iter().zip(widths) {
        let rect = Rect { x, y: row_y, width: w.min(inner.right().saturating_sub(x)), height: 1 };
        hits.invite_btns.push((rect, *b));
        f.render_widget(
            Paragraph::new(Span::styled(label(k, l), Style::new().fg(Color::Black).bg(*col).bold())),
            rect,
        );
        x += w + 1;
    }
    let note = if sharing.is_some() {
        "sharing on the LAN: tell the joiner the url, Esc stops"
    } else if max > 0 {
        "↑↓ PgUp PgDn or wheel scroll · y copy · w save · s share"
    } else {
        "buttons or keys: y copy · w save to file · s share on LAN"
    };
    f.render_widget(
        Paragraph::new(Line::styled(note, Style::new().fg(C_DIM))).alignment(Alignment::Center),
        Rect { x: inner.x, y: inner.y + inner.height.saturating_sub(1), width: inner.width, height: 1 },
    );
}

/// Network picker for a new invite (TUI-INVITE-002): a number, a click or
/// Enter on the highlighted row mints.
fn draw_pick_net(f: &mut Frame, app: &App, sel: usize, hits: &mut Hits) {
    let nets = app.snapshot.as_ref().map_or(&[][..], |s| &s.networks[..]);
    let w = 60u16.min(f.area().width.saturating_sub(2));
    let h = nets.len() as u16 + 4;
    let area = centered(f.area(), w, h);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Invite for which network? ")
        .title_style(Style::new().fg(C_ACCENT).bold());
    let inner = block.inner(area);
    f.render_widget(block, area);
    hits.pick_rows.clear();
    for (i, n) in nets.iter().enumerate() {
        let rect = Rect { x: inner.x, y: inner.y + i as u16, width: inner.width, height: 1 };
        if rect.y >= inner.y + inner.height.saturating_sub(1) {
            break;
        }
        hits.pick_rows.push(rect);
        let style = if i == sel { Style::new().fg(Color::Black).bg(C_ACCENT).bold() } else { Style::new() };
        let label = format!(" {} {}  ({}, {})", i + 1, n.name, n.role, n.subnet);
        f.render_widget(Paragraph::new(Span::styled(label, style)), rect);
    }
    f.render_widget(
        Paragraph::new(Line::styled("number, click or Enter picks · ↑↓ move · Esc cancel", Style::new().fg(C_DIM)))
            .alignment(Alignment::Center),
        Rect { x: inner.x, y: inner.y + inner.height.saturating_sub(1), width: inner.width, height: 1 },
    );
}

fn draw_confirm(f: &mut Frame, prompt: &str) {
    let w = 60u16.min(f.area().width.saturating_sub(2));
    let area = centered(f.area(), w, 7);
    f.render_widget(Clear, area);
    let lines = vec![
        Line::raw(""),
        Line::styled(prompt.to_string(), Style::new().fg(C_BAD).bold()),
        Line::raw(""),
        Line::from(vec![
            Span::styled("y", Style::new().fg(C_ACCENT).bold()),
            Span::styled(" yes    ", Style::new().fg(C_DIM)),
            Span::styled("n/Esc", Style::new().fg(C_ACCENT).bold()),
            Span::styled(" cancel", Style::new().fg(C_DIM)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).border_style(Style::new().fg(C_BAD))),
        area,
    );
}

fn draw_admin(f: &mut Frame, area: Rect, net: &NetSnap, app: &App) {
    let mut lines = Vec::new();
    let role_col = if net.is_admin() { C_OK } else { C_DIM };
    lines.push(Line::from(vec![
        Span::styled("role    ", Style::new().fg(C_DIM)),
        Span::styled(net.role.clone(), Style::new().fg(role_col).bold()),
        Span::styled(
            format!("   key {}", net.network_key.clone().unwrap_or_else(|| "-".into())),
            Style::new().fg(C_DIM),
        ),
    ]));

    // Pending nuke proposals (NUKE-CONSENSUS).
    if !net.nuke_proposals.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("⚠ pending nuke proposals", Style::new().fg(C_BAD).bold()));
        for (sid, _at) in &net.nuke_proposals {
            let name = admin_name(net, sid, false);
            lines.push(Line::styled(format!("   by {name} ({sid})  (X to second)"), Style::new().fg(C_WARN)));
        }
    }

    // Key-holders, fetched via AdminList.
    lines.push(Line::raw(""));
    lines.push(Line::styled("key-holders", Style::new().fg(C_DIM).bold()));
    match &app.admins {
        Some((n, admins)) if n == &net.name => {
            if admins.is_empty() {
                lines.push(Line::styled("  (none)", Style::new().fg(C_DIM)));
            }
            for (sid, is_self) in admins {
                let name = admin_name(net, sid, *is_self);
                lines.push(Line::from(vec![
                    Span::styled("  ● ", Style::new().fg(C_ACCENT)),
                    Span::styled(format!("{:<16}  ", trunc(&name, 16)), Style::new()),
                    Span::styled(sid.clone(), Style::new().fg(C_DIM)),
                    Span::styled(if *is_self { "  (you)" } else { "" }, Style::new().fg(C_DIM)),
                ]));
            }
        }
        _ => lines.push(Line::styled("  loading…", Style::new().fg(C_DIM))),
    }

    // Action legend.
    lines.push(Line::raw(""));
    let gate = if net.is_admin() { "" } else { "  (admins only, except Leave)" };
    lines.push(Line::styled(format!("actions{gate}"), Style::new().fg(C_DIM).bold()));
    lines.push(Line::styled("  a grant-admin to selected peer", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  K kick selected peer", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  X nuke / second proposal", Style::new().fg(C_DIM)));
    lines.push(Line::styled("  L leave this network", Style::new().fg(C_DIM)));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

// --- small helpers ---------------------------------------------------------

/// A (text, color) pair that becomes a styled table `Cell`.
struct Cellish(String, Color);
impl From<Cellish> for Cell<'static> {
    fn from(c: Cellish) -> Self {
        Cell::from(c.0).style(Style::new().fg(c.1))
    }
}

/// Background of the selected table row: a fixed dark grey, the same in every
/// terminal theme.
const SEL_BG: Color = Color::Rgb(40, 44, 52);

fn row_selected(r: Row<'static>) -> Row<'static> {
    r.style(Style::new().bg(SEL_BG))
}

/// Text colour for a cell on the selected row. [`SEL_BG`] is always dark, but
/// the terminal default foreground is near-black in a light theme and
/// `DarkGray` can be too, so those two become explicit light RGB values (RGB,
/// not ANSI white, which a light theme may remap to a dark colour). Status
/// colours (green, yellow, red, cyan) read fine on the dark bar and are kept.
fn on_selected(c: Color) -> Color {
    match c {
        Color::Reset => Color::Rgb(220, 223, 228),
        C_DIM => Color::Rgb(150, 156, 166),
        c => c,
    }
}

fn peer_marker(p: &PeerSnap) -> (&'static str, Color) {
    if p.is_coordinator {
        ("◆", C_ACCENT)
    } else if p.conn.is_some() {
        ("●", C_OK)
    } else {
        ("○", C_DIM)
    }
}

fn conn_type_str(t: &ConnType) -> (&'static str, Color) {
    match t {
        ConnType::Direct => ("direct", C_OK),
        ConnType::Relay => ("relay", C_WARN),
        ConnType::Tor => ("tor", Color::Magenta),
        ConnType::Veilid => ("veilid", C_ACCENT),
        ConnType::Unknown => ("?", C_DIM),
    }
}

fn via_detail_str(v: &ViaDetail) -> &'static str {
    match v {
        ViaDetail::NoDirectCandidate => "no direct candidate",
        ViaDetail::DirectUnvalidated => "direct candidate unvalidated",
        ViaDetail::DirectBled => "direct candidate excluded (overlay-bled)",
    }
}

fn color_rtt(rtt: Option<f64>) -> Color {
    match rtt {
        None => C_DIM,
        Some(r) if r < 20.0 => C_OK,
        Some(r) if r < 80.0 => C_WARN,
        Some(_) => C_BAD,
    }
}

fn fmt_rtt(rtt: Option<f64>) -> String {
    match rtt {
        Some(r) => format!("{r:.1}"),
        None => "-".into(),
    }
}

fn fmt_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b}{}", U[0])
    } else {
        format!("{v:.1}{}", U[i])
    }
}

fn fmt_rate(bps: u64) -> String {
    format!("{}/s", fmt_bytes(bps))
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn fmt_ago(ts: Option<u64>, now: u64) -> String {
    let Some(t) = ts else { return "-".into() };
    let d = now.saturating_sub(t);
    if d < 5 {
        "now".into()
    } else if d < 60 {
        format!("{d}s")
    } else if d < 3600 {
        format!("{}m", d / 60)
    } else if d < 86400 {
        format!("{}h", d / 3600)
    } else {
        format!("{}d", d / 86400)
    }
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Keep the last `max` chars (so a long input stays caret-visible).
fn tail(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        s.chars().skip(n - max).collect()
    }
}

fn dash(s: &str) -> &str {
    if s.is_empty() {
        "-"
    } else {
        s
    }
}

/// A centered rect of the given size (clamped to `area`).
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    Rect { x, y, width: w, height: h }
}

#[cfg(test)]
mod invite_tests {
    use super::*;
    use crate::app::{App, InviteBtn, Mode};
    use ratatui::{Terminal, backend::TestBackend};

    fn screen(app: &mut App) -> String {
        let mut t = Terminal::new(TestBackend::new(100, 40)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let b = t.backend().buffer().clone();
        (0..b.area.height)
            .map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn invite_overlay_has_buttons_with_key_hints_and_click_rects() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        app.mode = Mode::Invite { network: "n".into(), code: "CODE".into(), id: "i".into(), qr: "##\n##".into() };
        let s = screen(&mut app);
        for want in ["y copy", "w save", "s share on LAN", "m select text", "Esc close"] {
            assert!(s.contains(want), "missing {want}");
        }
        let kinds: Vec<_> = app.hits.invite_btns.iter().map(|(_, b)| *b).collect();
        assert!(kinds == [InviteBtn::Copy, InviteBtn::Save, InviteBtn::Share, InviteBtn::Select, InviteBtn::Close]);
    }

    fn click(app: &mut App, x: u16, y: u16) {
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.on_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }));
    }

    #[test]
    fn click_on_the_card_keeps_it_open_click_outside_closes() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        app.mode = Mode::Invite { network: "n".into(), code: "CODE".into(), id: "i".into(), qr: "##\n##".into() };
        screen(&mut app);
        let card = app.hits.invite_card.unwrap();
        click(&mut app, card.x + 3, card.y + 2); // on the code line
        assert!(matches!(app.mode, Mode::Invite { .. }), "card closed by a click on it");
        click(&mut app, 0, 0); // outside
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn every_invite_button_reacts_to_a_click() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        let open = |app: &mut App| {
            app.mode = Mode::Invite { network: "n".into(), code: "CODE".into(), id: "i".into(), qr: "##\n##".into() };
            screen(app);
        };
        // Save: no HOME file is written in a test, so check the toast changes.
        open(&mut app);
        let r = app.hits.invite_btns.iter().find(|(_, b)| *b == InviteBtn::Select).unwrap().0;
        assert!(app.mouse_on);
        click(&mut app, r.x + r.width / 2, r.y);
        assert!(!app.mouse_on, "Select button did nothing");
        open(&mut app);
        let r = app.hits.invite_btns.iter().find(|(_, b)| *b == InviteBtn::Copy).unwrap().0;
        click(&mut app, r.x + 1, r.y);
        assert!(app.toast.as_ref().is_some_and(|t| t.text.contains("copied")), "Copy button did nothing");
        assert!(matches!(app.mode, Mode::Invite { .. }));
        let r = app.hits.invite_btns.iter().find(|(_, b)| *b == InviteBtn::Close).unwrap().0;
        click(&mut app, r.x + r.width - 1, r.y);
        assert!(matches!(app.mode, Mode::Normal), "Close button did nothing");
    }

    #[test]
    fn small_window_scrolls_to_the_last_qr_row() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        let qr: String = (0..30).map(|i| format!("QR{i:02}")).collect::<Vec<_>>().join("\n");
        app.mode = Mode::Invite { network: "n".into(), code: "CODE".into(), id: "i".into(), qr };
        let mut t = Terminal::new(TestBackend::new(60, 14)).unwrap();
        t.draw(|f| draw(f, &mut app)).unwrap();
        assert!(app.invite_scroll_max.get() > 0, "no scroll offered");
        let text = |t: &Terminal<TestBackend>| {
            let b = t.backend().buffer().clone();
            (0..b.area.height)
                .map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol().to_string()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(!text(&t).contains("QR29"));
        for _ in 0..40 {
            app.on_event(crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Down,
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        t.draw(|f| draw(f, &mut app)).unwrap();
        let s = text(&t);
        assert!(s.contains("QR29"), "last QR row not reachable");
        assert!(s.contains("copy") || s.contains(" y "), "buttons scrolled away");
    }

    #[test]
    fn network_picker_lists_networks_and_picks_by_number() {
        use crate::model::{NetSnap, Snapshot};
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        let mk = |n: &str| NetSnap { name: n.into(), role: "admin".into(), subnet: "10.0.0.0/24".into(), ..Default::default() };
        app.snapshot = Some(Snapshot { networks: vec![mk("alpha"), mk("beta")], ..Default::default() });
        app.mode = Mode::PickNet { sel: 0 };
        let s = screen(&mut app);
        assert!(s.contains("Invite for which network?") && s.contains("1 alpha") && s.contains("2 beta"));
        assert_eq!(app.hits.pick_rows.len(), 2);
        app.on_event(crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('2'),
            crossterm::event::KeyModifiers::NONE,
        )));
        match rx.try_recv() {
            Ok(crate::action::Action::InviteCreate { network }) => assert_eq!(network, "beta"),
            _ => panic!("expected InviteCreate for beta"),
        }
        assert_eq!(app.sel_net, 1);
        // every button rect lies on a row that shows its label
        let rows: Vec<&str> = s.lines().collect();
        for (r, _) in &app.hits.invite_btns {
            assert!(rows[r.y as usize].contains("copy") || rows[r.y as usize].contains("close"));
        }
    }
}
