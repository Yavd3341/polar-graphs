use std::fs::create_dir_all;

use sfml::graphics::{
    Color, FloatRect, Font, PrimitiveType, RenderStates, RenderTarget, RenderTexture, RenderWindow,
    Text, Transformable, Vertex, View,
};
use sfml::system::{Clock, Vector2f, Vector2u};
use sfml::window::{ContextSettings, Event, Key, Style, VideoMode};
use sfml::SfBox;

use bitflags::bitflags;

bitflags! {
    pub struct Flags: u8 {
        const NO_DRAW = 1 << 0;
        const FULLSCREEN = 1 << 1;
        const PAUSE = 1 << 2;
        const FONT_FAILURE = 1 << 3;
        const DRAW_GUI = 1 << 4;
        const SHOW_CURSOR = 1 << 5;
        const RENDER_ANIMATION = 1 << 6;
        const NO_CUTOFF = 1 << 7;
    }
}

pub struct App {
    pub fps_clock: SfBox<Clock>,
    pub ctx_settings: ContextSettings,

    pub flags: Flags,
    pub font: Option<SfBox<Font>>,

    pub debug_text: String,

    pub background: Color,

    pub window: Option<RenderWindow>,
    pub size: Vector2u,
    pub fps_limit: u32,

    pub angle: f32,
    pub angle_limit: f32,
    pub angle_delta: f32,

    pub vertecies: Vec<Vertex>,
    pub desired_count: usize,

    pub plugin_init: fn(&mut Self),
    pub plgin_angle_to_point: fn(point: &mut sfml::system::Vector2f, angle: f32),

    render_texture: Option<RenderTexture>,
    pub render_texture_size: Vector2u,
    render_failures: u8,
    render_frame: u32,
}

impl App {
    //
    // Lifecycle code
    //

    pub fn new() -> App {
        App {
            fps_clock: Clock::start(),
            ctx_settings: ContextSettings::default(),
            flags: Flags::empty(),
            font: None,
            debug_text: String::new(),
            background: Color {
                r: 0,
                g: 0,
                b: 25,
                a: 255,
            },
            window: None,
            size: (800, 600).into(),
            fps_limit: 60,
            angle: 0.0,
            angle_limit: 360.0,
            angle_delta: 1.0,
            vertecies: Vec::new(),
            desired_count: 361,
            plugin_init: |app| app.angle_limit = 360.0,
            plgin_angle_to_point: |point, angle| {
                let (sin, cos) = angle.to_radians().sin_cos();
                point.x = cos;
                point.y = sin;
            },

            render_texture: None,
            render_texture_size: (1024, 1024).into(),
            render_failures: 0,
            render_frame: 0,
        }
    }

    pub fn init(&mut self, full: bool) {
        if full {
            self.flags = Flags::SHOW_CURSOR;

            if let Some(font) = Font::from_file("font.ttf") {
                self.font = Some(font);
            } else {
                self.flags |= Flags::FONT_FAILURE;
            }

            (self.plugin_init)(self);
            self.resize_data_array();
            self.reset_data_array();

            self.ctx_settings.antialiasing_level = 8;
        }

        if self.window.is_some() {
            let window = self.window.as_mut().unwrap();
            if window.is_open() {
                window.close()
            }
        }

        let mut window = RenderWindow::new(
            if self.flags.contains(Flags::FULLSCREEN) {
                VideoMode::desktop_mode()
            } else {
                VideoMode::from((self.size.x, self.size.y))
            },
            "Polar Roses",
            if self.flags.contains(Flags::FULLSCREEN) {
                Style::FULLSCREEN
            } else {
                Style::DEFAULT
            },
            &self.ctx_settings,
        );
        window.set_framerate_limit(self.fps_limit);
        window.set_mouse_cursor_visible(self.flags.contains(Flags::SHOW_CURSOR));

        self.window = Some(window);
    }

    pub fn run(&mut self) {
        if self.window.is_none() {
            self.init(true);
        }

        'main_loop: while self.window.as_ref().unwrap().is_open() {
            {
                while let Some(event) = self.window.as_mut().unwrap().poll_event() {
                    match event {
                        Event::Closed => {
                            self.close();
                            break 'main_loop;
                        }
                        Event::KeyPressed {
                            code,
                            ctrl,
                            shift,
                            alt,
                            ..
                        } => {
                            if !self.process_key(code, ctrl, shift, alt) {
                                break 'main_loop;
                            }
                        }
                        Event::Resized { width, height } => {
                            let new_width = width.max(300);
                            let new_height = height.max(300);

                            if !self.flags.contains(Flags::RENDER_ANIMATION) {
                                Self::rescale_data_array(
                                    &mut self.vertecies,
                                    self.size,
                                    (new_width, new_height).into(),
                                );
                            }

                            let window = self.window.as_mut().unwrap();
                            window.set_view(&View::from_rect(FloatRect::new(
                                0.0,
                                0.0,
                                new_width as f32,
                                new_height as f32,
                            )));
                            if width.min(height) < 300 {
                                window.set_size((new_width, new_height));
                                if !self.flags.contains(Flags::RENDER_ANIMATION) {
                                    self.size = window.size();
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }

            if self.flags.contains(Flags::RENDER_ANIMATION) {
                if !self.request_draw_texture() {
                    continue;
                }
            } else {
                self.size = self.window.as_ref().unwrap().size();

                if !self.flags.contains(Flags::PAUSE) {
                    self.request_update();
                }
            }

            self.request_draw();
            self.window.as_mut().unwrap().display();
        }
    }

    //
    // Input processing code
    //

    pub fn process_key(&mut self, code: Key, ctrl: bool, shift: bool, alt: bool) -> bool {
        if !self.flags.contains(Flags::RENDER_ANIMATION) {
            match code {
                Key::Escape => {
                    self.close();
                    return false;
                }
                Key::Space => self.flags.toggle(Flags::PAUSE),
                Key::F3 => self.flags.toggle(Flags::DRAW_GUI),
                Key::F5 => self.init(!shift),
                Key::H => {
                    self.flags.toggle(Flags::SHOW_CURSOR);
                    self.window
                        .as_mut()
                        .unwrap()
                        .set_mouse_cursor_visible(self.flags.contains(Flags::SHOW_CURSOR));
                }
                Key::G => {
                    self.prepare_render_texture();
                    match create_dir_all("out") {
                        Ok(()) => self.flags.insert(Flags::RENDER_ANIMATION),
                        Err(error) => eprintln!("{}", error),
                    }
                }
                Key::F2 => {
                    self.desired_count = (self.angle_limit / self.angle_delta).round() as usize + 1;
                    self.prepare_render_texture();
                    let is_no_cutoff = self.flags.contains(Flags::NO_CUTOFF);
                    self.flags.insert(Flags::NO_CUTOFF);
                    self.disable_cutoff();
                    self.draw_frame_to_texture("frame.png");
                    self.flags.set(Flags::NO_CUTOFF, is_no_cutoff);
                    self.size = self.window.as_ref().unwrap().size();
                    Self::rescale_data_array(
                        &mut self.vertecies,
                        self.render_texture_size,
                        self.size,
                    );
                }
                Key::C => self.reset_data_array(),
                Key::S => {
                    while self.angle < self.angle_limit {
                        self.angle += self.angle_delta;
                        self.update_data_array();
                    }
                }
                Key::F | Key::F11 => {
                    self.flags.toggle(Flags::FULLSCREEN);
                    self.init(false);
                    Self::rescale_data_array(
                        &mut self.vertecies,
                        self.size,
                        self.window.as_ref().unwrap().size(),
                    );
                }
                Key::N => {
                    self.flags.toggle(Flags::NO_CUTOFF);
                    self.disable_cutoff();
                }
                Key::RBracket => {
                    if self.ctx_settings.antialiasing_level < 16 {
                        self.ctx_settings.antialiasing_level += 1;
                    }
                }
                Key::LBracket => {
                    if self.ctx_settings.antialiasing_level > 0 {
                        self.ctx_settings.antialiasing_level -= 1;
                    }
                }
                Key::Num9 => {
                    if shift {
                        let step = if alt { 1 } else { 10 };
                        self.fps_limit -= if self.fps_limit < step {
                            self.fps_limit
                        } else {
                            step
                        };
                        self.window
                            .as_mut()
                            .unwrap()
                            .set_framerate_limit(self.fps_limit);
                    }
                }
                Key::Num0 => {
                    if shift {
                        // Due to Windows behaviour Shift + Ctrl + 0 is
                        // not being registered thus Alt key is used here
                        // and also in Num9 section for consistency
                        self.fps_limit += if alt { 1 } else { 10 };
                        self.window
                            .as_mut()
                            .unwrap()
                            .set_framerate_limit(self.fps_limit);
                    }
                }
                Key::Add | Key::Equal => {
                    if ctrl {
                        self.angle_delta += Self::get_shift_multiplier() * 0.1;
                    } else {
                        self.desired_count += Self::get_shift_multiplier() as usize;
                    }
                }
                Key::Subtract | Key::Hyphen => {
                    if ctrl {
                        self.angle_delta -= Self::get_shift_multiplier() * 0.1;
                    } else {
                        let delta = Self::get_shift_multiplier() as usize;
                        self.desired_count -= if self.desired_count < delta {
                            self.desired_count
                        } else {
                            delta
                        };
                    }
                }
                _ => (),
            }
        }
        true
    }

    pub fn get_shift_multiplier() -> f32 {
        let mut mult = 1.0;

        if Key::is_pressed(Key::LShift) {
            mult *= 10.0;
        }

        if Key::is_pressed(Key::RShift) {
            mult *= 10.0;
        }

        mult
    }

    fn close(&mut self) {
        self.window.as_mut().unwrap().close();
    }

    //
    // Update code
    //

    pub fn request_update(&mut self) {
        let fps = self.get_fps();

        if !self.flags.contains(Flags::PAUSE) {
            self.angle += self.angle_delta;
            self.angle %= self.angle_limit;
        }

        self.resize_data_array();

        self.debug_text = format!(
            include_str!("debug_screen_template.txt"),
            fps,
            if self.fps_limit > 0 {
                format!(
                    " (max: {} - {:6.2}%)",
                    self.fps_limit,
                    fps / self.fps_limit as f32 * 100.0
                )
            } else {
                "".to_owned()
            },
            if self.flags.contains(Flags::PAUSE) {
                "[paused]"
            } else {
                ""
            },
            self.angle,
            self.angle_limit,
            self.angle_delta,
            self.vertecies.len(),
            self.size.x,
            self.size.y,
            self.window.as_ref().unwrap().settings().antialiasing_level,
            self.ctx_settings.antialiasing_level,
            self.flags.bits
        );

        self.update_data_array();
    }

    fn get_fps(&mut self) -> f32 {
        let current_time = self.fps_clock.restart().as_seconds();
        1.0 / current_time
    }

    fn prepare_render_texture(&mut self) {
        self.size = self.render_texture_size;
        self.resize_data_array();
        self.reset_data_array();

        self.angle = 0.0;
        while self.angle < self.angle_limit {
            self.angle += self.angle_delta;
            self.update_data_array();
        }
        self.angle = 0.0;

        self.render_texture = RenderTexture::with_settings(
            self.render_texture_size.x,
            self.render_texture_size.y,
            &self.ctx_settings,
        );
        self.render_failures = 0;
        self.render_frame = 0;
    }

    //
    // Draw code
    //

    pub fn request_draw(&mut self) {
        let render_target = self.window.as_mut().unwrap();
        Self::draw_frame(render_target, self.background, &self.vertecies);
        if self.flags.contains(Flags::DRAW_GUI) && !self.flags.contains(Flags::FONT_FAILURE) {
            let mut debug_label = Text::new(&self.debug_text, self.font.as_ref().unwrap(), 16);
            debug_label.set_fill_color(Color::WHITE);
            debug_label.set_outline_color(self.background);
            debug_label.set_outline_thickness(1.5);
            debug_label.set_position((10.0, 10.0));
            render_target.draw(&debug_label);
        }
    }

    pub fn request_draw_texture(&mut self) -> bool {
        let mut fps = 0.0;
        match self.render_failures {
            0 => {
                self.angle += self.angle_delta;
                self.update_data_array();
                fps = self.get_fps();
            }
            10 => {
                self.flags.remove(Flags::RENDER_ANIMATION);
                println!("Drawing failed on frame {:5}{:35}", self.render_frame, "");
                return false;
            }
            _ => (),
        }

        if self.angle > self.angle_limit {
            self.size = self.window.as_ref().unwrap().size();
            self.reset_data_array();
            self.flags.remove(Flags::RENDER_ANIMATION);
            println!(
                "Drawing finished with {:5} frames{:30}",
                self.render_frame, ""
            );
            return false;
        }

        print!(
            "Drawing frame {:5} out of {:5} (fps: {:10.5}, failures: {:2})\r",
            self.render_frame,
            (self.angle_limit / self.angle_delta).ceil() as u32,
            fps,
            self.render_failures
        );
        if self.draw_frame_to_texture(&format!("out/frame-{}.png", self.render_frame)) {
            self.render_failures = 0;
            self.render_frame += 1;
        } else {
            self.render_failures += 1;
        }
        true
    }

    pub fn draw_frame(
        render_target: &mut dyn RenderTarget,
        background: Color,
        vertecies: &Vec<Vertex>,
    ) {
        render_target.clear(background);
        render_target.draw_primitives(vertecies, PrimitiveType::LINE_STRIP, &RenderStates::DEFAULT);
    }

    fn draw_frame_to_texture(&mut self, filename: &str) -> bool {
        let render_texture = self.render_texture.as_mut().unwrap();
        Self::draw_frame(render_texture, self.background, &self.vertecies);
        render_texture.display();

        render_texture
            .texture()
            .copy_to_image()
            .as_ref()
            .unwrap()
            .save_to_file(filename)
    }

    //
    // Data array manipulation code
    //

    fn disable_cutoff(&mut self) {
        if self.flags.contains(Flags::NO_CUTOFF) {
            self.vertecies
                .iter_mut()
                .for_each(|vertex| vertex.color.a = 0xFF)
        }
    }

    pub fn update_data_array(&mut self) {
        self.vertecies.rotate_left(1);

        let len_last = self.vertecies.len() - 1;

        if !self.flags.contains(Flags::NO_CUTOFF) {
            let index_to_alpha = 1.0 / len_last as f32 * 255.0;
            self.vertecies
                .iter_mut()
                .enumerate()
                .for_each(|(i, vertex)| vertex.color.a = (i as f32 * index_to_alpha) as u8);
        }

        (self.plgin_angle_to_point)(&mut self.vertecies[len_last].position, self.angle);
        Self::unit_to_screen_point(&mut self.vertecies[len_last].position, self.size);
    }

    pub fn resize_data_array(&mut self) {
        if self.desired_count != self.vertecies.len() {
            let old_len = self.vertecies.len();

            if self.desired_count < old_len {
                self.vertecies.drain(0..old_len - self.desired_count);
                self.vertecies.shrink_to_fit();
            } else {
                self.vertecies.reserve(self.desired_count);

                if old_len == 0 {
                    self.vertecies.push(Vertex::new(
                        (0.0, 0.0).into(),
                        Color::WHITE,
                        (0.0, 0.0).into(),
                    ));
                    (self.plgin_angle_to_point)(&mut self.vertecies[0].position, 0.0);
                    Self::unit_to_screen_point(&mut self.vertecies[0].position, self.size);
                }

                let last_elem = self.vertecies.last().unwrap().clone();
                while self.vertecies.len() < self.desired_count {
                    self.vertecies.push(last_elem.clone());
                }
            }
        }
    }

    pub fn reset_data_array(&mut self) {
        self.angle = 0.0;
        for vertex in self.vertecies.iter_mut() {
            (self.plgin_angle_to_point)(&mut vertex.position, self.angle);
            Self::unit_to_screen_point(&mut vertex.position, self.size);
        }
    }

    pub fn rescale_data_array(vertecies: &mut Vec<Vertex>, old_size: Vector2u, new_size: Vector2u) {
        let old_radius = Self::get_radius(&old_size);
        let new_radius = Self::get_radius(&new_size);

        for vertex in vertecies.iter_mut() {
            vertex.position.x = (vertex.position.x - old_size.x as f32 / 2.0) / old_radius
                * new_radius
                + new_size.x as f32 / 2.0;
            vertex.position.y = (vertex.position.y - old_size.y as f32 / 2.0) / old_radius
                * new_radius
                + new_size.y as f32 / 2.0;
        }
    }

    pub fn get_radius(size: &Vector2u) -> f32 {
        size.x.min(size.y) as f32 / 2.0 - 50.0
    }

    pub fn unit_to_screen_point(point: &mut Vector2f, size: Vector2u) {
        let radius = Self::get_radius(&size);
        point.x = point.x * radius + size.x as f32 / 2.0;
        point.y = -point.y * radius + size.y as f32 / 2.0;
    }
}
