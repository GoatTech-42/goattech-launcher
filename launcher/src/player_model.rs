//! The home stage's 3D player: the skin's texture projected onto the
//! player's cuboids and software-rasterized - orthographic, yaw plus a slight downward pitch,
//! z-buffered, nearest texels (the pixel look). Classic and slim account models, with a
//! labelled fixture preview when no supported account texture is available.

/// One face of a cuboid: 3D origin + per-texel u/v vectors (model space, y up, z toward the
/// viewer) and its rectangle in the 64x64 skin texture.
struct Face {
    o: [f32; 3],
    u: [f32; 3],
    v: [f32; 3],
    tex: (usize, usize, usize, usize), // x, y, w, h
}

/// A cuboid at x x0..x1, y y0..y1, z z0..z1 with base and overlay texture columns. `bx,by` is
/// the base column's top-left in the skin (top rect at bx+? per the standard unwrap), `ox,oy`
/// the overlay column's. The standard unwrap: top (bx+depth..), right (bx), front (bx+depth),
/// left (bx+depth+width), back (bx+2*depth+width), each at row by+depth.
struct Part {
    x0: f32,
    y0: f32,
    z0: f32,
    w: f32,
    h: f32,
    d: f32,
    bx: usize,
    by: usize,
    ox: usize,
    oy: usize,
}

impl Part {
    fn faces(&self, inflate: f32) -> [(bool, Face); 4] {
        let (x0, y0, z0) = (self.x0 - inflate, self.y0 - inflate, self.z0 - inflate);
        let (w, h, d) = (self.w + 2. * inflate, self.h + 2. * inflate, self.d + 2. * inflate);
        let (wi, hi, di) = (self.w as usize, self.h as usize, self.d as usize);
        let face = |o: [f32; 3], u: [f32; 3], v: [f32; 3], tex: (usize, usize, usize, usize)| Face {
            o,
            u,
            v,
            tex,
        };
        [
            // front: z near, u +x, v -y
            (
                true,
                face(
                    [x0, y0 + h, z0 + d],
                    [w, 0., 0.],
                    [0., -h, 0.],
                    (self.bx + di, self.by + di, wi, hi),
                ),
            ),
            // top: y far, u +x, v +z (texture bottom edge meets the front's top)
            (
                true,
                face(
                    [x0, y0 + h, z0],
                    [w, 0., 0.],
                    [0., 0., d],
                    (self.bx + di, self.by, wi, di),
                ),
            ),
            // player's right side (x near): u +z, v -y
            (
                true,
                face(
                    [x0, y0 + h, z0],
                    [0., 0., d],
                    [0., -h, 0.],
                    (self.bx, self.by + di, di, hi),
                ),
            ),
            // player's left side (x far): u -z, v -y
            (
                true,
                face(
                    [x0 + w, y0 + h, z0 + d],
                    [0., 0., -d],
                    [0., -h, 0.],
                    (self.bx + di + wi, self.by + di, di, hi),
                ),
            ),
        ]
    }

    fn overlay_faces(&self) -> [(bool, Face); 4] {
        let mut f = self.faces(0.5);
        // Overlay region may sit left of / above the base region: shift with signed math.
        let du = self.ox as i64 - self.bx as i64;
        let dv = self.oy as i64 - self.by as i64;
        for (_, face) in f.iter_mut() {
            face.tex.0 = (face.tex.0 as i64 + du) as usize;
            face.tex.1 = (face.tex.1 as i64 + dv) as usize;
        }
        f
    }
}

/// The six parts in model space (texels; y up, 0 at the feet; z toward the viewer): legs,
/// body, arms, head. Standard wide layout columns in the 64x64 skin.
fn parts(slim: bool) -> [Part; 6] {
    let mut parts = [
        // right leg (viewer's left), x -4..0
        Part { x0: -4., y0: 0., z0: -2., w: 4., h: 12., d: 4., bx: 0, by: 16, ox: 0, oy: 32 },
        // left leg, x 0..4
        Part { x0: 0., y0: 0., z0: -2., w: 4., h: 12., d: 4., bx: 16, by: 48, ox: 0, oy: 48 },
        // body, x -4..4
        Part { x0: -4., y0: 12., z0: -2., w: 8., h: 12., d: 4., bx: 16, by: 16, ox: 16, oy: 32 },
        // right arm, x -8..-4
        Part { x0: -8., y0: 12., z0: -2., w: 4., h: 12., d: 4., bx: 40, by: 16, ox: 40, oy: 32 },
        // left arm, x 4..8
        Part { x0: 4., y0: 12., z0: -2., w: 4., h: 12., d: 4., bx: 32, by: 48, ox: 48, oy: 48 },
        // head
        Part { x0: -4., y0: 24., z0: -4., w: 8., h: 8., d: 8., bx: 0, by: 0, ox: 32, oy: 0 },
    ];
    if slim {
        parts[3].x0 = -7.;
        parts[3].w = 3.;
        parts[4].w = 3.;
    }
    parts
}


/// A limb pose: swing angles (radians around X at the shoulder/hip) and the head's yaw.
/// Positive swing moves the limb forward (toward the viewer).
#[derive(Clone, Copy, Default)]
pub struct Pose {
    pub arm_r: f32,
    pub arm_l: f32,
    pub leg_r: f32,
    pub leg_l: f32,
    pub head_yaw: f32,
}

impl Pose {
    /// Relaxed idle: slow arm sway and an occasional look around.
    pub fn idle(t: f32) -> Pose {
        Pose {
            arm_r: 0.09 * (t * 1.7).sin(),
            arm_l: -0.09 * (t * 1.7).sin(),
            leg_r: 0.,
            leg_l: 0.,
            head_yaw: 0.35 * (t * 0.45).sin(),
        }
    }

    /// Waving: the right arm is up and oscillates; the rest keeps a light sway.
    pub fn wave(t: f32) -> Pose {
        let mut p = Pose::idle(t);
        p.arm_r = std::f32::consts::PI - 0.45 * (t * 7.0).sin();
        p
    }
}

/// Rotate `v` around the X axis.
fn rx(v: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c]
}

/// Rotate `v` around the Y axis.
fn ry(v: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

/// Swing a face around `pivot`: origin rotates around the point, u/v around the origin.
fn swing(f: &mut Face, pivot: [f32; 3], a: f32, around_y: bool) {
    let r = |v| if around_y { ry(v, a) } else { rx(v, a) };
    let d = [f.o[0] - pivot[0], f.o[1] - pivot[1], f.o[2] - pivot[2]];
    let d = r(d);
    f.o = [pivot[0] + d[0], pivot[1] + d[1], pivot[2] + d[2]];
    f.u = r(f.u);
    f.v = r(f.v);
}

/// Render size in texels before `art::bake` scales it.
pub const W: usize = 40;
pub const H: usize = 44;

/// The fixture player at `yaw` (radians, positive turns the player's left side toward the
/// viewer) with a slight downward pitch: straight-alpha RGBA, transparent background.
pub fn render(skin: &image::RgbaImage, yaw: f32) -> Vec<[u8; 4]> {
    render_model(skin, yaw, false)
}

fn render_model(skin: &image::RgbaImage, yaw: f32, slim: bool) -> Vec<[u8; 4]> {
    render_posed(skin, yaw, 0.21, &Pose::default(), slim)
}

/// The model at `yaw` and `pitch` in `pose`: what the interactive home stage draws each frame.
pub fn render_posed(skin: &image::RgbaImage, yaw: f32, pitch: f32, pose: &Pose, slim: bool) -> Vec<[u8; 4]> {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    // Model space: +z toward the viewer. Projected depth: smaller = closer (z-buffer order).
    let project = |p: [f32; 3]| -> (f32, f32, f32) {
        let x = p[0] * cy - p[2] * sy;
        let z1 = p[0] * sy + p[2] * cy;
        let y = p[1] * cp - z1 * sp;
        let z = p[1] * sp + z1 * cp;
        (x, -y, -z)
    };
    let (cx, feet) = (W as f32 / 2., H as f32 - 2.); // feet sit 2 texels above the floor
    let mut buf = vec![[0u8; 4]; W * H];
    let mut depth = vec![f32::INFINITY; W * H];
    let mut draw_face = |f: &Face, skin: &image::RgbaImage| {
        // Backface cull: the rotated normal must point toward the viewer (+z in model terms).
        let n = [
            f.u[1] * f.v[2] - f.u[2] * f.v[1],
            f.u[2] * f.v[0] - f.u[0] * f.v[2],
            f.u[0] * f.v[1] - f.u[1] * f.v[0],
        ];
        let z1 = n[0] * sy + n[2] * cy;
        // (v runs down with y up, so every face normal above comes out negated)
        if n[1] * sp + z1 * cp >= 0. {
            return;
        }
        let (tw, th) = (f.tex.2 as f32, f.tex.3 as f32);
        let (ox, oy, od) = project(f.o);
        // Per-texel steps are directions through the same linear map.
        let dir = |v: [f32; 3]| {
            let x = v[0] * cy - v[2] * sy;
            let z1 = v[0] * sy + v[2] * cy;
            let y = v[1] * cp - z1 * sp;
            let z = v[1] * sp + z1 * cp;
            (x, -y, -z)
        };
        let su = dir([f.u[0] / tw, f.u[1] / tw, f.u[2] / tw]);
        let sv = dir([f.v[0] / th, f.v[1] / th, f.v[2] / th]);
        for tv in 0..f.tex.3 {
            for tu in 0..f.tex.2 {
                let sx = cx + ox + tu as f32 * su.0 + tv as f32 * sv.0;
                let syy = feet + oy + tu as f32 * su.1 + tv as f32 * sv.1;
                let sd = od + tu as f32 * su.2 + tv as f32 * sv.2;
                let (px, py) = (sx.round() as i32, syy.round() as i32);
                if px < 0 || py < 0 || px >= W as i32 || py >= H as i32 {
                    continue;
                }
                let idx = py as usize * W + px as usize;
                if sd >= depth[idx] {
                    continue;
                }
                let c = skin.get_pixel((f.tex.0 + tu) as u32, (f.tex.1 + tv) as u32).0;
                if c[3] == 0 {
                    continue;
                }
                depth[idx] = sd;
                buf[idx] = c;
            }
        }
    };
    // Limb pivots in model space: shoulders at the arm's top center, hips at the leg's top,
    // the neck under the head. Arms and legs swing around X; the head turns around Y.
    let all = parts(slim);
    let pivot = |i: usize, p: &Part| match i {
        3 | 4 => [p.x0 + p.w / 2., 23., 0.],
        0 | 1 => [p.x0 + p.w / 2., 12., 0.],
        _ => [0., 24., 0.],
    };
    let angle = |i: usize| match i {
        0 => (pose.leg_r, false),
        1 => (pose.leg_l, false),
        3 => (pose.arm_r, false),
        4 => (pose.arm_l, false),
        5 => (pose.head_yaw, true),
        _ => (0., false),
    };
    for layer in 0..2 {
        for (i, part) in all.iter().enumerate() {
            let mut faces = if layer == 0 { part.faces(0.) } else { part.overlay_faces() };
            let (a, around_y) = angle(i);
            if a != 0. {
                let piv = pivot(i, part);
                for (on, f) in faces.iter_mut() {
                    if *on {
                        swing(f, piv, a, around_y);
                    }
                }
            }
            for (on, f) in faces.iter() {
                if *on {
                    draw_face(f, skin);
                }
            }
        }
    }
    buf
}

/// The embedded fixture skin, decoded.
pub fn fixture() -> Option<image::RgbaImage> {
    image::load_from_memory(include_bytes!("../assets/fixture-skin.png"))
        .ok()
        .map(|i| i.to_rgba8())
}

/// Render an account model; unsupported layouts keep the labelled fallback.
pub fn render_account(skin: &image::RgbaImage, yaw: f32, slim: bool) -> Option<Vec<[u8; 4]>> {
    if skin.dimensions() != (64, 64) {
        return None;
    }
    Some(render_model(skin, yaw, slim))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_render_uses_the_supplied_texture_not_the_fixture() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([17, 33, 99, 255]));
        let rendered = render_account(&skin, -0.5, false).unwrap();
        let pixels: Vec<_> = rendered.iter().filter(|p| p[3] > 0).collect();
        assert!(pixels.len() > 200);
        assert!(pixels.iter().all(|p| **p == [17, 33, 99, 255]));
    }

    #[test]
    fn slim_arms_use_three_texels_not_four() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([80, 100, 120, 255]));
        let classic = render_account(&skin, 0., false).unwrap();
        let slim = render_account(&skin, 0., true).unwrap();
        assert_ne!(classic, slim);
        let count = |p: &Vec<[u8; 4]>| p.iter().filter(|p| p[3] > 0).count();
        assert!(count(&slim) < count(&classic));
    }

    #[test]
    fn unsupported_skin_dimensions_do_not_panic_or_claim_a_real_model() {
        assert!(render_account(&image::RgbaImage::new(64, 32), 0., false).is_none());
        assert!(render_account(&image::RgbaImage::new(32, 32), 0., false).is_none());
    }

    #[test]
    fn renders_opaque_player_pixels() {
        let skin = fixture().expect("fixture skin");
        let buf = render(&skin, -0.5);
        let opaque = buf.iter().filter(|c| c[3] > 0).count();
        assert!(opaque > 200, "expected a solid player, got {opaque} texels");
    }

    #[test]
    fn posed_limbs_change_the_silhouette() {
        let skin = fixture().expect("fixture skin");
        let plain = render_posed(&skin, 0., 0.21, &Pose::default(), false);
        let mut waving = Pose::default();
        waving.arm_r = std::f32::consts::PI;
        let waved = render_posed(&skin, 0., 0.21, &waving, false);
        assert_ne!(plain, waved, "a raised arm must change the picture");
    }

    #[test]
    fn wave_lifts_arm_pixels_above_the_shoulder_line() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 40, 40, 255]));
        let mut waving = Pose::default();
        waving.arm_r = std::f32::consts::PI;
        let buf = render_posed(&skin, 0., 0.21, &waving, false);
        // Arm texels (skin column 40..44 base / overlay) land red everywhere;
        // a raised arm puts opaque pixels above the head's top rows.
        let top_opaque = buf[..super::W * 9].iter().filter(|p| p[3] > 0).count();
        assert!(top_opaque > 0, "raised arm should reach the top rows");
    }

    #[test]
    fn yaw_turns_the_side_into_view() {
        let skin = fixture().expect("fixture skin");
        let front = render(&skin, 0.0);
        let turned = render(&skin, -0.6);
        assert_ne!(front, turned, "yaw should change the silhouette");
    }
}
