mod viewer;

use viewer::App;

const N: u8 = 4;
const D: u8 = 5;
const K: f32 = N as f32 / D as f32;

fn main() {
    let mut app = App::new();

    app.plugin_init = |app| {
        app.angle_limit = 180.0 * if N % 2 == D % 2 { D } else { 2 * D } as f32;
        app.desired_count = (app.angle_limit / app.angle_delta).round() as usize + 1;
    };

    app.plgin_angle_to_point = |point, angle| {
        let rad = angle.to_radians();
        let multiplier = (rad * K).cos();
        let (rad_sin, rad_cos) = rad.sin_cos();

        point.x = rad_cos * multiplier;
        point.y = rad_sin * multiplier;
    };

    app.run();
}
