//! The home stage's 3D player (`assets/fixture-skin.png`): the skin's texture projected onto the
//! player's cuboids and software-rasterized - orthographic, yaw plus a slight downward pitch,
//! z-buffered, nearest texels (the pixel look). No GPU, no skin service: a labelled fixture
//! preview until the real account skin renders here. Wide (4-texel arm) layout only.

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
fn parts() -> [Part; 6] {
    [
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
    ]
}

/// Render size in texels before `art::bake` scales it.
pub const W: usize = 40;
pub const H: usize = 44;

/// The fixture player at `yaw` (radians, positive turns the player's left side toward the
/// viewer) with a slight downward pitch: straight-alpha RGBA, transparent background.
pub fn render(skin: &image::RgbaImage, yaw: f32) -> Vec<[u8; 4]> {
    let pitch = 0.21_f32; // ~12 degrees: looks down at the model, tops show
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
    for part in parts() {
        for (on, f) in part.faces(0.) {
            if on {
                draw_face(&f, skin);
            }
        }
    }
    for part in parts() {
        for (on, f) in part.overlay_faces() {
            if on {
                draw_face(&f, skin);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_opaque_player_pixels() {
        let skin = fixture().expect("fixture skin");
        let buf = render(&skin, -0.5);
        let opaque = buf.iter().filter(|c| c[3] > 0).count();
        assert!(opaque > 200, "expected a solid player, got {opaque} texels");
    }

    #[test]
    fn yaw_turns_the_side_into_view() {
        let skin = fixture().expect("fixture skin");
        let front = render(&skin, 0.0);
        let turned = render(&skin, -0.6);
        assert_ne!(front, turned, "yaw should change the silhouette");
    }
}
