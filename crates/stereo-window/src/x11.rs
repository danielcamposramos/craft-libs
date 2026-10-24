//! The X11 window: a plain window showing the picture, declared with window properties while it
//! shows a pair.
//!
//! The properties are the ones Sparky Stereo OS's stereo declaration defines for X11 (version 2 of the root
//! window's `_KDE_NET_WM_STEREO_CONTENT_SUPPORTED`): `_KDE_NET_WM_STEREO_CONTENT` holds the
//! packing (3, full side by side, left eye first). From version 3 the compositor doubles the window of a program that
//! declares itself, so the window is created at one eye's size; before that the program makes the
//! window as wide as its two eyes. Declared pictures retain their native raster; the compositor owns their display size.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, ImageOrder, PropMode, Screen, Window,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::{Change, Command, Declaration, Error, Picture, SIDE_BY_SIDE_FULL, Shared, convert_row, fit, plan};

/// Connects, creates the window and starts its thread.
pub(crate) fn open(title: &str, app_id: &str, shared: Arc<Shared>) -> Result<(mpsc::Sender<Command>, JoinHandle<()>), Error> {
    let (tx, rx) = mpsc::channel::<Command>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), Error>>();
    let (title, app_id) = (title.to_string(), app_id.to_string());
    let thread = std::thread::Builder::new()
        .name("stereo-window".into())
        .spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&title, &app_id, &shared, &rx, &ready_tx)));
            if outcome.is_err() {
                let _ = ready_tx.send(Err(Error::X11("the window thread panicked".into())));
            }
            shared.alive.store(false, Ordering::Release);
            shared.declared.store(false, Ordering::Release);
        })
        .map_err(|e| Error::X11(e.to_string()))?;
    match ready_rx.recv() {
        Ok(Ok(())) => Ok((tx, thread)),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(Error::X11("the window thread ended before it was ready".into()))
        }
    }
}

fn x11<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> Error + '_ {
    move |e| Error::X11(format!("{what}: {e}"))
}

struct Display {
    conn: RustConnection,
    window: Window,
    root: Window,
    pixmap: Option<(u32, u32, u32)>,
    placement: Placement,
    gc: u32,
    depth: u8,
    msb_first: bool,
    /// The compositor's declaration version (0: none).
    support: u32,
    atoms: Atoms,
}

#[derive(Clone, Copy)]
struct Atoms {
    content: u32,
    supported: u32,
    delete: u32,
    protocols: u32,
    net_wm_name: u32,
    utf8: u32,
    one_view_size: u32,
    configure: u32,
    commit: u32,
}

fn atoms(conn: &RustConnection) -> Result<Atoms, Error> {
    let intern =
        |name: &[u8]| -> Result<u32, Error> { Ok(conn.intern_atom(false, name).map_err(x11("intern"))?.reply().map_err(x11("intern"))?.atom) };
    Ok(Atoms {
        content: intern(b"_KDE_NET_WM_STEREO_CONTENT")?,
        supported: intern(b"_KDE_NET_WM_STEREO_CONTENT_SUPPORTED")?,
        delete: intern(b"WM_DELETE_WINDOW")?,
        protocols: intern(b"WM_PROTOCOLS")?,
        net_wm_name: intern(b"_NET_WM_NAME")?,
        utf8: intern(b"UTF8_STRING")?,
        one_view_size: intern(b"_KDE_NET_WM_STEREO_CONTENT_ONE_VIEW_SIZE")?,
        configure: intern(b"_KDE_NET_WM_STEREO_CONTENT_ONE_VIEW_CONFIGURE")?,
        commit: intern(b"_KDE_NET_WM_STEREO_CONTENT_ONE_VIEW_COMMIT")?,
    })
}

/// The declaration version the root window announces, 0 when it does not.
fn support(conn: &RustConnection, root: Window, atom: u32) -> u32 {
    conn.get_property(false, root, atom, AtomEnum::CARDINAL, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().and_then(|mut v| v.next()))
        .unwrap_or(0)
}

/// A 32-bit-per-pixel format for the screen's depth, or an error.
fn check_format(conn: &RustConnection, screen: &Screen) -> Result<(u8, bool), Error> {
    let setup = conn.setup();
    let ok = setup.pixmap_formats.iter().any(|f| f.depth == screen.root_depth && f.bits_per_pixel == 32);
    if !ok {
        return Err(Error::X11(format!("the screen's depth {} has no 32-bit pixel format", screen.root_depth)));
    }
    Ok((screen.root_depth, setup.image_byte_order == ImageOrder::MSB_FIRST))
}

fn run(title: &str, app_id: &str, shared: &Arc<Shared>, commands: &mpsc::Receiver<Command>, ready: &mpsc::Sender<Result<(), Error>>) {
    let mut display = match setup(title, app_id) {
        Ok(d) => d,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    shared.desktop_declares.store(display.support >= 2, Ordering::Release);
    shared.alive.store(true, Ordering::Release);
    let _ = ready.send(Ok(()));

    let mut picture: Option<Picture> = None;
    let mut declaration = Declaration::default();
    let mut size = (0u32, 0u32);
    let mut mapped = false;
    let mut resize_needed = false;
    let mut proposal = None;
    'outer: loop {
        // events first, then a wait on the commands (also the pace of the loop)
        let mut redraw = false;
        loop {
            match display.conn.poll_for_event() {
                Ok(Some(Event::ConfigureNotify(e))) => {
                    // the synthetic copy carries the program's own size, not the window's
                    if e.response_type & 0x80 == 0 {
                        size = (u32::from(e.width), u32::from(e.height));
                        redraw = true;
                    }
                }
                Ok(Some(Event::PropertyNotify(e)))
                    if e.atom == display.atoms.configure && display.support >= 7 && picture.as_ref().is_some_and(Picture::is_pair) =>
                {
                    proposal = None;
                    match display.proposal() {
                        Ok(p) => proposal = p,
                        Err(e) => log::warn!("stereo window: {e}"),
                    }
                    redraw = true;
                }
                Ok(Some(Event::Expose(_))) => redraw = true,
                Ok(Some(Event::DestroyNotify(e))) if e.window == display.window => break 'outer,
                Ok(Some(Event::ClientMessage(m))) if m.data.as_data32()[0] == display.atoms.delete => {
                    break 'outer;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    log::warn!("stereo window: {e}");
                    break 'outer;
                }
            }
        }
        match commands.recv_timeout(Duration::from_millis(15)) {
            Ok(Command::Show(p)) => {
                resize_needed = mapped && picture.as_ref().is_none_or(|old| (old.width, old.height, old.stereo) != (p.width, p.height, p.stereo));
                picture = Some(p);
                redraw = true;
            }
            Ok(Command::Close) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if redraw && let Some(p) = &picture {
            let decided = plan(p.is_pair(), display.support >= 2);
            match declaration.update(decided.declare) {
                Some(Change::Declare) => {
                    proposal = None;
                    display.declare(true);
                    shared.declared.store(true, Ordering::Release);
                }
                Some(Change::Remove) => {
                    proposal = None;
                    display.declare(false);
                    shared.declared.store(false, Ordering::Release);
                }
                None => {}
            }
            if !mapped {
                if decided.declare
                    && let Err(e) = display.place(p, None)
                {
                    log::warn!("stereo window: {e}");
                    break;
                }
                if let Err(e) = display.map(p) {
                    log::warn!("stereo window: {e}");
                    break;
                }
                mapped = true;
                size = initial_size(&display, p);
            } else if resize_needed {
                if let Err(e) = display.resize(p) {
                    log::warn!("stereo window: {e}");
                    break;
                }
                resize_needed = false;
            }
            if decided.declare {
                // The declaration doubles native X11 geometry asynchronously. Never squeeze a
                // pair into the old drawable while waiting for that ConfigureNotify.
                let actual = display.conn.get_geometry(display.window).ok().and_then(|c| c.reply().ok());
                if !actual.is_some_and(|r| usize::from(r.width) == p.width && usize::from(r.height) == p.height) {
                    continue;
                }
            }
            if let Err(e) = display.draw(p, decided.left_eye_only, size, decided.declare, proposal.take()) {
                log::warn!("stereo window: {e}");
                break;
            }
        }
    }
    display.declare(false);
    if let Some((pixmap, _, _)) = display.pixmap {
        let _ = display.conn.free_pixmap(pixmap);
    }
    let _ = display.conn.destroy_window(display.window);
    let _ = display.conn.flush();
}

/// A declared pair asks for its native eye raster (both eyes on support version 2).
/// The ordinary desktop fallback retains its fitted window size.
fn window_size(p: &Picture, support: u32) -> (u32, u32) {
    let (w, h) = if p.is_pair() && support >= 2 {
        (u32::try_from(p.eye_width()).unwrap_or(u32::MAX), u32::try_from(p.height).unwrap_or(u32::MAX))
    } else {
        fit(p.eye_width(), p.height, None)
    };
    if p.is_pair() && (2..3).contains(&support) { (w.saturating_mul(2), h) } else { (w, h) }
}

fn initial_size(display: &Display, p: &Picture) -> (u32, u32) {
    window_size(p, display.support)
}

fn setup(title: &str, app_id: &str) -> Result<Display, Error> {
    let (conn, screen_num) = x11rb::connect(None).map_err(x11("connect"))?;
    let screen = conn.setup().roots.get(screen_num).ok_or_else(|| Error::X11("no screen".into()))?.clone();
    let (depth, msb_first) = check_format(&conn, &screen)?;
    let atoms = atoms(&conn)?;
    let support = support(&conn, screen.root, atoms.supported);
    let window = conn.generate_id().map_err(x11("window id"))?;
    let gc = conn.generate_id().map_err(x11("gc id"))?;
    let aux = CreateWindowAux::new()
        .background_pixel(screen.black_pixel)
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
    conn.create_window(depth, window, screen.root, 0, 0, 640, 360, 0, WindowClass::INPUT_OUTPUT, screen.root_visual, &aux)
        .map_err(x11("create window"))?;
    conn.create_gc(gc, window, &CreateGCAux::new()).map_err(x11("create gc"))?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes()).map_err(x11("title"))?;
    conn.change_property8(PropMode::REPLACE, window, atoms.net_wm_name, atoms.utf8, title.as_bytes()).map_err(x11("title"))?;
    let class = format!("{app_id}\0{app_id}\0");
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, class.as_bytes()).map_err(x11("class"))?;
    conn.change_property32(PropMode::REPLACE, window, atoms.protocols, AtomEnum::ATOM, &[atoms.delete]).map_err(x11("protocols"))?;
    conn.flush().map_err(x11("flush"))?;
    Ok(Display { conn, window, root: screen.root, pixmap: None, placement: Placement::default(), gc, depth, msb_first, support, atoms })
}

impl Display {
    /// Sizes the window for the first picture and shows it.
    fn map(&mut self, p: &Picture) -> Result<(), Error> {
        let (w, h) = window_size(p, self.support);
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new().width(w).height(h);
        self.conn.configure_window(self.window, &aux).map_err(x11("size"))?;
        self.conn.map_window(self.window).map_err(x11("map"))?;
        self.conn.flush().map_err(x11("flush"))
    }

    fn resize(&self, p: &Picture) -> Result<(), Error> {
        let (w, h) = window_size(p, self.support);
        self.conn
            .configure_window(self.window, &x11rb::protocol::xproto::ConfigureWindowAux::new().width(w).height(h))
            .map_err(x11("size"))?
            .check()
            .map_err(x11("size"))?;
        Ok(())
    }

    fn proposal(&self) -> Result<Option<(u32, u32, u32)>, Error> {
        let r = self
            .conn
            .get_property(false, self.window, self.atoms.configure, AtomEnum::CARDINAL, 0, 3)
            .map_err(x11("size"))?
            .reply()
            .map_err(x11("size"))?;
        if r.type_ != u32::from(AtomEnum::CARDINAL) || r.format != 32 || r.bytes_after != 0 {
            return Ok(None);
        }
        let words: Vec<_> = r.value32().into_iter().flatten().collect();
        Ok(placement_proposal(self.support, &words))
    }

    fn place(&mut self, p: &Picture, proposal: Option<(u32, u32, u32)>) -> Result<(), Error> {
        if self.support < 6 {
            return Ok(());
        }
        let previous = self.placement.destination;
        let (w, h) = self.placement.size(p, proposal.map(|(_, w, h)| (w, h)));
        if previous == Some((w, h)) && proposal.is_none() {
            return Ok(());
        }
        self.conn
            .change_property32(PropMode::REPLACE, self.window, self.atoms.one_view_size, AtomEnum::CARDINAL, &[w, h])
            .map_err(x11("size"))?
            .check()
            .map_err(x11("size"))?;
        if let Some((serial, _, _)) = proposal {
            let message = ClientMessageEvent::new(32, self.window, self.atoms.commit, [serial, 0, 0, 0, 0]);
            self.conn
                .send_event(false, self.root, EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT, message)
                .map_err(x11("size"))?
                .check()
                .map_err(x11("size"))?;
        }
        Ok(())
    }

    /// Sets or deletes the declaration properties.
    fn declare(&mut self, on: bool) {
        let a = self.atoms;
        if !on {
            self.placement = Placement::default();
            let _ = self.conn.delete_property(self.window, a.one_view_size);
        }
        let sent = if on {
            self.conn.change_property32(PropMode::REPLACE, self.window, a.content, AtomEnum::CARDINAL, &[SIDE_BY_SIDE_FULL]).map(|_| ())
        } else {
            self.conn.delete_property(self.window, a.content).map(|_| ())
        };
        if let Err(e) = sent {
            log::warn!("stereo window: declaration: {e}");
        }
        let _ = self.conn.flush();
    }

    /// Uploads a native pair, or the ordinary desktop fallback, then presents one complete image.
    fn draw(&mut self, p: &Picture, left_eye_only: bool, size: (u32, u32), native: bool, proposal: Option<(u32, u32, u32)>) -> Result<(), Error> {
        let (dw, dh) = if native { (p.width, p.height) } else { (size.0 as usize, size.1 as usize) };
        // an X11 image is at most 65535 wide, and its bands are placed by a signed 16-bit row
        if dw == 0 || dh == 0 || dw > 0xFFFF || dh > 0x7FFF {
            return Err(Error::X11("the picture exceeds X11 drawable dimensions".into()));
        }
        let out = if native {
            native_pixels(p, self.msb_first)?
        } else {
            let src_w = if left_eye_only { p.eye_width() } else { p.width };
            // a row of the source at the destination's width, nearest neighbour
            let mut rgba_row = vec![0u8; dw * 4];
            let mut out = vec![0u8; dw * 4 * dh];
            let x_map: Vec<usize> = (0..dw).map(|x| (x * src_w / dw).min(src_w - 1)).collect();
            for (y, out_row) in out.chunks_exact_mut(dw * 4).enumerate() {
                let sy = (y * p.height / dh).min(p.height - 1);
                let src_row = p.rgba.get(sy * p.width * 4..(sy + 1) * p.width * 4).ok_or(Error::BadPicture)?;
                for (x, sx) in x_map.iter().enumerate() {
                    rgba_row[x * 4..x * 4 + 4].copy_from_slice(&src_row[sx * 4..sx * 4 + 4]);
                }
                convert_row(&rgba_row, out_row, self.msb_first);
            }
            out
        };
        if self.pixmap.is_none_or(|(_, w, h)| (w as usize, h as usize) != (dw, dh)) {
            let pixmap = self.conn.generate_id().map_err(x11("pixmap id"))?;
            self.conn.create_pixmap(self.depth, pixmap, self.window, dw as u16, dh as u16).map_err(x11("pixmap"))?.check().map_err(x11("pixmap"))?;
            if let Some((old, _, _)) = self.pixmap.replace((pixmap, dw as u32, dh as u32)) {
                self.conn.free_pixmap(old).map_err(x11("pixmap"))?;
            }
        }
        let Some((pixmap, _, _)) = self.pixmap else {
            return Err(Error::BadPicture);
        };
        // PutImage in bands that fit the server's request size
        let max = self.conn.maximum_request_bytes().saturating_sub(64).max(4096);
        let band = (max / (dw * 4)).clamp(1, dh);
        for (i, chunk) in out.chunks(band * dw * 4).enumerate() {
            let rows = chunk.len() / (dw * 4);
            self.conn
                .put_image(ImageFormat::Z_PIXMAP, pixmap, self.gc, dw as u16, rows as u16, 0, (i * band) as i16, 0, self.depth, chunk)
                .map_err(x11("put image"))?;
        }
        if native {
            self.place(p, proposal)?;
        }
        self.conn.copy_area(pixmap, self.window, self.gc, 0, 0, 0, 0, dw as u16, dh as u16).map_err(x11("put image"))?;
        self.conn.flush().map_err(x11("flush"))
    }
}

fn native_pixels(p: &Picture, msb_first: bool) -> Result<Vec<u8>, Error> {
    let stride = p.width.checked_mul(4).ok_or(Error::BadPicture)?;
    let len = stride.checked_mul(p.height).ok_or(Error::BadPicture)?;
    if stride == 0 || p.height == 0 || len != p.rgba.len() {
        return Err(Error::BadPicture);
    }
    let mut pixels = vec![0; len];
    for (source, destination) in p.rgba.chunks_exact(stride).zip(pixels.chunks_exact_mut(stride)) {
        convert_row(source, destination, msb_first);
    }
    Ok(pixels)
}

#[derive(Default)]
struct Placement {
    destination: Option<(u32, u32)>,
    aspect: Option<f64>,
}

impl Placement {
    fn size(&mut self, p: &Picture, proposal: Option<(u32, u32)>) -> (u32, u32) {
        let aspect = p.eye_width() as f64 / p.height as f64;
        if proposal.is_none()
            && self.aspect == Some(aspect)
            && let Some(size) = self.destination
        {
            return size;
        }
        let size = fit(p.eye_width(), p.height, proposal.or(self.destination));
        self.destination = Some(size);
        self.aspect = Some(aspect);
        size
    }
}

fn placement_proposal(support: u32, words: &[u32]) -> Option<(u32, u32, u32)> {
    let [serial, width, height] = words else {
        return None;
    };
    (support >= 7 && *serial != 0 && (1..=32767).contains(width) && (1..=32767).contains(height)).then_some((*serial, *width, *height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_window_keeps_the_native_eye_raster() {
        let picture = Picture::new(3072, 540, vec![0; 3072 * 540 * 4].into(), true).unwrap();
        for version in [3, 6, 7] {
            assert_eq!(window_size(&picture, version), (1536, 540), "support {version}");
        }
        assert_eq!(window_size(&picture, 2), (3072, 540));
        assert_eq!(window_size(&picture, 0), (1280, 450), "the ordinary desktop fallback is unchanged");
    }

    #[test]
    fn a_rounded_place_is_stable_until_the_shape_or_proposal_changes() {
        let p = Picture::new(3072, 540, vec![0; 3072 * 540 * 4].into(), true).unwrap();
        let mut place = Placement::default();
        assert_eq!(place.size(&p, Some((900, 700))), (900, 316));
        for _ in 0..20 {
            assert_eq!(place.size(&p, None), (900, 316));
        }
        assert_eq!(place.size(&p, Some((1200, 700))), (1200, 422));
        let square = Picture::new(1080, 540, vec![0; 1080 * 540 * 4].into(), true).unwrap();
        assert_eq!(place.size(&square, None), (422, 422));
    }
    #[test]
    fn only_a_complete_current_protocol_proposal_can_be_acknowledged() {
        assert_eq!(placement_proposal(7, &[1, 1600, 900]), Some((1, 1600, 900)));
        assert_eq!(placement_proposal(8, &[u32::MAX, 1, 32767]), Some((u32::MAX, 1, 32767)));
        for words in [&[][..], &[1], &[1, 2], &[1, 2, 3, 4], &[0, 1600, 900], &[1, 0, 900], &[1, 1600, 0], &[1, 32768, 900], &[1, 1600, 32768]] {
            assert_eq!(placement_proposal(7, words), None, "{words:?}");
        }
        assert_eq!(placement_proposal(6, &[1, 1600, 900]), None);
    }

    #[test]
    fn upload_keeps_every_native_eye_sample() {
        let (w, h) = (3072, 540);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[(x % 251) as u8, (y % 253) as u8, (x / (w / 2) * 127) as u8, 255]);
            }
        }
        let picture = Picture::new(w, h, rgba.clone().into(), true).unwrap();
        for msb_first in [false, true] {
            let out = native_pixels(&picture, msb_first).unwrap();
            let width = w;
            assert_eq!(out.len(), width * h * 4);
            for y in 0..h {
                for x in 0..width {
                    let src = &rgba[(y * w + x) * 4..][..4];
                    let expected = if msb_first { [255, src[0], src[1], src[2]] } else { [src[2], src[1], src[0], 255] };
                    assert_eq!(&out[(y * width + x) * 4..][..4], &expected);
                }
            }
        }
    }
}
