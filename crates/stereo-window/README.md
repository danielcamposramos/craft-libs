# craft-stereo-window

The Stereo View window: a native window that shows a stereo pair and declares it to the
desktop. Standalone (L0), safe Rust, MIT OR Apache-2.0. Shared by PhotoCraft and FilmCraft desktop adapters.

**One format, declared once.** The caller delivers a stereo picture as full side by side, left eye
first, each eye at its own resolution. The window tells the desktop that this is what its surface holds,
and only while it shows a pair:

| Windowing system | Declaration |
|---|---|
| Wayland | `kde_stereo_content_v1.set_content(side_by_side_full)` (value 3) on the window's surface. `wp_viewporter` gives the surface the size of one eye (a declared surface is placed at one view's size); the buffer is the whole pair. Global announced: `kde_stereo_content_manager_v1`. |
| X11 | the window property `_KDE_NET_WM_STEREO_CONTENT` (3), when the root window announces `_KDE_NET_WM_STEREO_CONTENT_SUPPORTED` version 2 or later. From version 3 the compositor doubles the window of a program that declares itself, so the window is created at one eye's size. |

Declared X11 pictures keep their native raster, including when it is larger than the window's
place on screen. From support version 6, `ONE_VIEW_SIZE` gives that place independently of the
samples. Version 7 resize proposals are acknowledged with their serial after a complete native
pair is ready, then presented with one pixmap copy. The compositor scales the eyes to their place.

The desktop decides what each screen shows (a display's 3D mode, a virtual 3D mode, or the left
eye on a screen that is 2D). The window has no output formats and no anaglyph: it never mixes the
eyes. Where the desktop announces no declaration, the window shows the left eye only, so two
squeezed pictures never appear. A picture that is not a pair is shown as it is and never
declared; the declaration is removed when a pair gives way to one.

`protocols/kde-stereo-content-v1.xml` is the protocol description of Sparky Stereo OS's
`libstereo-declare` order495 protocol at675798f6, vendored byte for byte (SPDX MIT-CMU, Daniel Campos Ramos; a
permissive licence, kept as it is). It has one packing, `side_by_side_full` (3), and no class
request. The crate's own code is MIT OR Apache-2.0.

Everything runs on the window's own thread and display connection, with the Wayland client
libraries (`smithay-client-toolkit`, `wayland-client`, MIT) and `x11rb` (MIT OR Apache-2.0), no
`unsafe`.

`Picture::with_pixel_aspect_ratio` carries the intended width/height of a source sample.
It changes the Wayland viewport destination and X11 one-view place, never the uploaded buffer.
For example, two 960×1080 half-width views remain a 1920×1080 pair with 2:1 samples.
On X11, non-square samples require support version 6's independent ONE_VIEW_SIZE; older servers show the ordinary left-eye preview instead.
Square-sample pairs retain the existing version 2/3 compatibility path.


## Scene depth limits

`StereoWindow::depth_limits` returns the compositor's current limits together with the logical output/view geometry, without changing media-pair delivery.
Wayland consumes version5 `kde_stereo_depth_v1.depth_limits`; X11 support10 consumes `_KDE_NET_WM_STEREO_DEPTH_LIMITS`, exactly two nonnegative24.8fixed-point CARDINALs.
`DepthLimits::native_pixels` converts the1920-wide-output convention into the source raster's samples, accounting for the output/view width ratio once.
It never resizes a picture and exposes no app-specific strength, device, separation or output option.
An older desktop, missing/malformed limits, unknown geometry or an ambiguous Wayland surface spanning several outputs returns `None`.
Callers must say why depth is unavailable instead of inventing a budget; media whose two eyes already exist does not need this API.
