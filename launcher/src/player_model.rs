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
    fn faces(&self, inflate: f32) -> [(bool, Face); 6] {
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
            // back (z far): u -x, v -y - a limb swung up or a turned model shows it
            (
                true,
                face(
                    [x0 + w, y0 + h, z0],
                    [-w, 0., 0.],
                    [0., -h, 0.],
                    (self.bx + 2 * di + wi, self.by + di, wi, hi),
                ),
            ),
            // bottom (y near): u +x, v -z
            (
                true,
                face(
                    [x0, y0, z0 + d],
                    [w, 0., 0.],
                    [0., 0., -d],
                    (self.bx + 2 * di, self.by, wi, di),
                ),
            ),
        ]
    }

    fn overlay_faces(&self) -> [(bool, Face); 6] {
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

/// The model at `yaw` and `pitch` in `pose`, at the small W x H grid (tests and callers that
/// want model-texel resolution). The on-screen stage uses `render_posed_hi`.
pub fn render_posed(skin: &image::RgbaImage, yaw: f32, pitch: f32, pose: &Pose, slim: bool) -> Vec<[u8; 4]> {
    render_grid(skin, yaw, pitch, pose, slim, W, H, false)
}

/// The model rasterized straight at display resolution: geometry edges, silhouette and
/// projection land on real output pixels while the skin is still sampled nearest (the pixel
/// look comes from the texture, not from a coarse raster). Subtle per-face shading keeps the
/// depth readable instead of a flat card.
pub fn render_posed_hi(
    skin: &image::RgbaImage,
    yaw: f32,
    pitch: f32,
    pose: &Pose,
    slim: bool,
    out_w: usize,
    out_h: usize,
) -> Vec<[u8; 4]> {
    render_grid(skin, yaw, pitch, pose, slim, out_w.max(1), out_h.max(1), true)
}

fn render_grid(
    skin: &image::RgbaImage,
    yaw: f32,
    pitch: f32,
    pose: &Pose,
    slim: bool,
    out_w: usize,
    out_h: usize,
    shade: bool,
) -> Vec<[u8; 4]> {
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
    // Model units per output pixel: the model is W x H texels, so the projected face
    // vectors below span `mag` output pixels per model texel.
    let mag = out_w as f32 / W as f32;
    let (cx, feet) = (out_w as f32 / 2., out_h as f32 - 2. * mag); // feet 2 texels above the floor
    let mut buf = vec![[0u8; 4]; out_w * out_h];
    let mut depth = vec![f32::INFINITY; out_w * out_h];
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
        // Directions through the same linear map.
        let dir = |v: [f32; 3]| {
            let x = v[0] * cy - v[2] * sy;
            let z1 = v[0] * sy + v[2] * cy;
            let y = v[1] * cp - z1 * sp;
            let z = v[1] * sp + z1 * cp;
            (x, -y, -z)
        };
        // The face's full u/v edges in output pixels (orthographic: the face is a
        // parallelogram on screen). su/sv span the whole edge, not one texel.
        let su = dir([f.u[0] * mag, f.u[1] * mag, f.u[2] * mag]);
        let sv = dir([f.v[0] * mag, f.v[1] * mag, f.v[2] * mag]);
        let det = su.0 * sv.1 - su.1 * sv.0;
        if det.abs() < 1e-6 {
            return; // edge-on face has no area on screen
        }
        // Face tone from its view-space normal: front faces brightest, edges darker,
        // so the cuboids read as 3D instead of a flat card. Subtle on purpose.
        let tone = if shade {
            let nv = dir([n[0], n[1], n[2]]);
            let len = (nv.0 * nv.0 + nv.1 * nv.1 + nv.2 * nv.2).sqrt().max(1e-6);
            let (nx, ny, nz) = (nv.0 / len, nv.1 / len, nv.2 / len);
            // Light from above-left-front in view space (y down, -z near).
            let l = (-0.35f32, -0.75f32, -0.55f32);
            let ll = (l.0 * l.0 + l.1 * l.1 + l.2 * l.2).sqrt();
            let d = (nx * l.0 + ny * l.1 + nz * l.2) / ll;
            (0.82 + 0.18 * d.abs()).min(1.)
        } else {
            1.
        };
        let (oxs, oys) = (cx + ox * mag, feet + oy * mag);
        // Bounding box of the parallelogram's four corners.
        let corners = [
            (oxs, oys),
            (oxs + su.0, oys + su.1),
            (oxs + sv.0, oys + sv.1),
            (oxs + su.0 + sv.0, oys + su.1 + sv.1),
        ];
        let min_x = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min).floor() as i32;
        let max_x = corners.iter().map(|c| c.0).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
        let min_y = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min).floor() as i32;
        let max_y = corners.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
        for py in min_y.max(0)..max_y.min(out_h as i32) {
            for px in min_x.max(0)..max_x.min(out_w as i32) {
                // Pixel center in face coordinates: o + a*u + b*v, a/b in 0..1.
                let (qx, qy) = (px as f32 + 0.5 - oxs, py as f32 + 0.5 - oys);
                let a = (qx * sv.1 - qy * sv.0) / det;
                let b = (su.0 * qy - su.1 * qx) / det;
                if !(0. ..=1.).contains(&a) || !(0. ..=1.).contains(&b) {
                    continue;
                }
                let sd = od + a * su.2 / mag + b * sv.2 / mag;
                let idx = py as usize * out_w + px as usize;
                if sd >= depth[idx] {
                    continue;
                }
                // Nearest skin texel: the blocky texture look survives, the coarse
                // raster staircase does not.
                let tu = ((a * tw) as usize).min(f.tex.2 - 1);
                let tv = ((b * th) as usize).min(f.tex.3 - 1);
                let c = skin.get_pixel((f.tex.0 + tu) as u32, (f.tex.1 + tv) as u32).0;
                if c[3] == 0 {
                    continue;
                }
                depth[idx] = sd;
                let t = |v: u8| (v as f32 * tone).round().min(255.) as u8;
                buf[idx] = [t(c[0]), t(c[1]), t(c[2]), c[3]];
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
        let top_row = |pose: &Pose| {
            let buf = render_posed(&skin, 0., 0.21, pose, false);
            (0..super::H).find(|&y| (0..super::W).any(|x| buf[y * super::W + x][3] > 0))
        };
        let mut waving = Pose::default();
        waving.arm_r = std::f32::consts::PI;
        let up = top_row(&waving).expect("raised arm is visible");
        let down = top_row(&Pose::default()).expect("head is visible");
        assert!(
            up < down,
            "a raised arm reaches above the head's top edge (arm up top row {up}, arms down top row {down})"
        );
    }

    /// Longest run of rows whose left silhouette edge sits at the same x. A coarse
    /// raster enlarged afterwards leaves texel-long stair steps; display-resolution
    /// rasterization breaks them up.
    fn longest_edge_plateau(buf: &[[u8; 4]], w: usize, h: usize) -> usize {
        let edge: Vec<Option<usize>> = (0..h)
            .map(|y| (0..w).find(|&x| buf[y * w + x][3] > 0))
            .collect();
        let mut longest = 0;
        let mut run = 1;
        for pair in edge.windows(2) {
            if pair[0].is_some() && pair[0] == pair[1] {
                run += 1;
                longest = longest.max(run);
            } else {
                run = 1;
            }
        }
        longest
    }

    #[test]
    fn display_resolution_edges_do_not_staircase_like_an_enlarged_coarse_raster() {
        let skin = fixture().expect("fixture skin");
        let yaw = 0.35;
        let hi = render_posed_hi(&skin, yaw, 0.21, &Pose::default(), false, W * 8, H * 8);
        // What the old pipeline produced: the coarse raster enlarged 8x nearest.
        let coarse = render_posed(&skin, yaw, 0.21, &Pose::default(), false);
        let mut enlarged = vec![[0u8; 4]; W * 8 * H * 8];
        for y in 0..H * 8 {
            for x in 0..W * 8 {
                enlarged[y * W * 8 + x] = coarse[(y / 8) * W + (x / 8)];
            }
        }
        let hi_plateau = longest_edge_plateau(&hi, W * 8, H * 8);
        let coarse_plateau = longest_edge_plateau(&enlarged, W * 8, H * 8);
        assert!(
            hi_plateau * 2 <= coarse_plateau.max(1) || hi_plateau <= 3,
            "hi-res edge plateau {hi_plateau} should be far below the enlarged coarse {coarse_plateau}"
        );
    }

    #[test]
    fn hi_res_shading_makes_a_flat_skin_read_as_3d() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([90, 140, 200, 255]));
        let buf = render_posed_hi(&skin, -0.5, 0.21, &Pose::default(), false, W * 8, H * 8);
        let tones: std::collections::HashSet<_> =
            buf.iter().filter(|p| p[3] > 0).map(|p| (p[0], p[1], p[2])).collect();
        assert!(tones.len() >= 2, "a turned model should show more than one face tone");
        // And the tones stay close to the skin color: shading is a hint, not a repaint.
        assert!(tones.iter().all(|&(r, g, b)| {
            (70..=100).contains(&r) && (110..=150).contains(&g) && (160..=200).contains(&b)
        }));
    }

    #[test]
    fn yaw_turns_the_side_into_view() {
        let skin = fixture().expect("fixture skin");
        let front = render(&skin, 0.0);
        let turned = render(&skin, -0.6);
        assert_ne!(front, turned, "yaw should change the silhouette");
    }
}
