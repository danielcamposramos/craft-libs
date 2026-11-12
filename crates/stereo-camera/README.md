# craft-stereo-camera

Coherent parallel eye cameras for a renderer that already owns a central camera and one world.
No desktop, UI or GPU dependency is required.

Pass the existing view, projection, physical eye, positive target depth and visible scene depth bounds and actual perspective environment horizontal focal coefficient to `Camera`.
`Budget` contains native-eye pixel magnitudes from the host, not a separation setting.
`pair()` returns left and right views without mutating any central state.
Perspective uses an off-axis frustum; orthographic uses a shear, with parallel axes in both cases.
The largest admissible baseline keeps near geometry within the popped budget and far geometry and the environment at infinity within the sunk budget.
`EyeCamera::background_view_projection` uses the same physical eyes when an orthographic scene retains a perspective environment.

For native width W, f=P00*W/2, eye distance e, positive central depth Z and target depth D, the left-minus-right pixel prediction is `e*f*(1/Z-1/D)` in perspective and `e*f*(1-Z/D)` in orthographic.
The target plane and vertical disparity are zero.
`DepthRange::from_aabb` projects bounded world geometry into central camera depth; the caller first includes any displacement envelope.
An invalid or incoherent camera returns an error.
A zero budget returns the exact central camera bits.

The renderer shares scene data and independent-light shadow maps, while each eye owns its view-dependent buffers, effects and histories.
A window host obtains live compositor limits, converts them into native samples, pairs equal scene states, and hands over one left-first full side-by-side picture.
This crate owns no output modes, eye swap or desktop policy.
