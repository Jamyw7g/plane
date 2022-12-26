use std::{
    collections::HashMap,
    ffi::c_void,
    time::{Duration, Instant},
};

use sdl2::{
    event::Event,
    image::{ImageRWops, InitFlag, Sdl2ImageContext},
    keyboard::Keycode,
    mixer::{
        self, allocate_channels, close_audio, open_audio, Channel, Chunk, Group, Sdl2MixerContext,
        DEFAULT_CHANNELS, DEFAULT_FORMAT, DEFAULT_FREQUENCY,
    },
    pixels::Color,
    rect::Rect,
    render::{CanvasBuilder, Texture, WindowCanvas},
    rwops::RWops,
    Sdl,
};

extern "C" {
    fn rand() -> u32;
    fn srand(_: u32);
    fn time(_: *const c_void) -> u64;
}

struct Player {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Player {
    fn new(w: i32, h: i32) -> Self {
        Self { x: 0, y: 0, w, h }
    }

    fn move_to(&mut self, x: i32, y: i32) {
        self.x = x - self.w / 2;
        self.y = y - self.h / 2;
    }
}

struct Bullet {
    instance: Instant,
    poss: Vec<(Rect, bool)>,
    w: i32,
    h: i32,
}

impl Bullet {
    fn new(w: i32, h: i32) -> Self {
        Self {
            instance: Instant::now(),
            poss: Vec::new(),
            w,
            h,
        }
    }

    fn produce(&mut self, play_pos: Rect) -> bool {
        let dur = self.instance.elapsed();
        let mut added = false;
        if dur >= Duration::from_secs_f32(0.35) {
            self.instance = Instant::now();
            let pos = Rect::new(
                play_pos.x + play_pos.width() as i32 / 2 - self.w / 2,
                play_pos.y - self.w,
                self.w as u32,
                self.h as u32,
            );
            self.poss.push((pos, true));
            added = true;
        }
        for (plane, _) in self.poss.iter_mut() {
            plane.y -= 2;
        }
        self.poss
            .retain(|(r, live)| r.y >= -(r.height() as i32) && *live);
        added
    }
}

struct Enemy {
    instance: Instant,
    planes: Vec<(Rect, bool)>,
    w: i32,
    h: i32,
    can_w: i32,
    can_h: i32,
}

impl Enemy {
    fn new(can_w: i32, can_h: i32, w: i32, h: i32) -> Self {
        Self {
            instance: Instant::now(),
            planes: Vec::new(),
            w,
            h,
            can_w,
            can_h,
        }
    }

    fn produce(&mut self) -> bool {
        let dur = self.instance.elapsed();
        let mut added = false;
        if dur >= Duration::from_secs_f32(0.65) {
            self.instance = Instant::now();

            let x = unsafe { rand() } % (self.can_w as u32).saturating_sub(self.w as u32);

            let pos = Rect::new(x as i32, 0, self.w as u32, self.h as u32);
            self.planes.push((pos, true));
            added = true;
        }
        for (plane, _) in self.planes.iter_mut() {
            plane.y += 1;
        }
        self.planes.retain(|(r, live)| r.y <= self.can_h && *live);
        added
    }
}

enum State {
    Start,
    Playing,
    Over,
}

pub struct Game {
    sdl: Sdl,
    _image_context: Sdl2ImageContext,
    _mixer_context: Sdl2MixerContext,
    state: State,
    score: usize,
    canvas: WindowCanvas,
    images: HashMap<String, Texture>,
    musics: HashMap<String, Chunk>,
    group: Group,
    enemy: Enemy,
    player: Player,
    bullet: Bullet,
}

impl Game {
    pub fn new() -> Self {
        let sdl = sdl2::init().unwrap();
        let image_context = sdl2::image::init(InitFlag::JPG | InitFlag::PNG).unwrap();
        unsafe {
            srand(time(std::ptr::null()) as u32);
        }
        let start_img = RWops::from_file("./res/start.jpg", "rb")
            .unwrap()
            .load_jpg()
            .unwrap();
        let over_img = RWops::from_file("./res/over.jpg", "rb")
            .unwrap()
            .load_jpg()
            .unwrap();
        let bg_img = RWops::from_file("./res/background.jpg", "rb")
            .unwrap()
            .load_jpg()
            .unwrap();
        let player_img = RWops::from_file("./res/player.png", "rb")
            .unwrap()
            .load_png()
            .unwrap();
        let enemy_img = RWops::from_file("./res/enemy.png", "rb")
            .unwrap()
            .load_png()
            .unwrap();
        let bullet_img = RWops::from_file("./res/bullet.png", "rb")
            .unwrap()
            .load_png()
            .unwrap();
        let can_w = start_img.width();
        let can_h = start_img.height();

        let player = Player::new(player_img.width() as i32, player_img.height() as i32);
        let enemy = Enemy::new(
            can_w as i32,
            can_h as i32,
            enemy_img.width() as i32,
            enemy_img.height() as i32,
        );
        let bullet = Bullet::new(bullet_img.width() as i32, bullet_img.height() as i32);

        let window = sdl
            .video()
            .unwrap()
            .window("Plane", can_w, can_h)
            .position_centered()
            .build()
            .unwrap();
        let canvas = CanvasBuilder::new(window)
            .accelerated()
            .present_vsync()
            .build()
            .unwrap();

        let txt_creator = canvas.texture_creator();
        let mut images = HashMap::new();
        images.insert(
            "start".to_string(),
            start_img.as_texture(&txt_creator).unwrap(),
        );
        images.insert(
            "over".to_string(),
            over_img.as_texture(&txt_creator).unwrap(),
        );
        images.insert(
            "background".to_string(),
            bg_img.as_texture(&txt_creator).unwrap(),
        );
        images.insert(
            "player".to_string(),
            player_img.as_texture(&txt_creator).unwrap(),
        );
        images.insert(
            "enemy".to_string(),
            enemy_img.as_texture(&txt_creator).unwrap(),
        );
        images.insert(
            "bullet".to_string(),
            bullet_img.as_texture(&txt_creator).unwrap(),
        );

        let mixer_context = sdl2::mixer::init(mixer::InitFlag::all()).unwrap();
        open_audio(DEFAULT_FREQUENCY, DEFAULT_FORMAT, DEFAULT_CHANNELS, 256).unwrap();
        allocate_channels(32);

        let mut musics = HashMap::new();
        let bullet_m = Chunk::from_file("./res/bullet.ogg").unwrap();
        musics.insert("bullet".to_string(), bullet_m);
        let enemy_down_m = Chunk::from_file("./res/enemy_down.ogg").unwrap();
        musics.insert("enemy_down".to_string(), enemy_down_m);
        let me_down_m = Chunk::from_file("./res/me_down.ogg").unwrap();
        musics.insert("me_down".to_string(), me_down_m);

        let bg_music = Chunk::from_file("./res/game_music.ogg").unwrap();
        Channel::all().play(&bg_music, -1).unwrap().set_volume(30);
        musics.insert("bg_music".to_string(), bg_music);

        Self {
            sdl,
            state: State::Start,
            _image_context: image_context,
            _mixer_context: mixer_context,
            score: 0,
            group: Group::default(),
            canvas,
            images,
            musics,
            enemy,
            player,
            bullet,
        }
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::RGB(128, 255, 128));
        self.canvas.clear();
        match self.state {
            State::Start => {
                self.canvas
                    .copy(self.images.get("start").unwrap(), None, None)
                    .unwrap();
            }
            State::Playing => {
                self.canvas
                    .copy(self.images.get("background").unwrap(), None, None)
                    .unwrap();
                if self.enemy.produce() {}
                let player_pos = Rect::new(
                    self.player.x,
                    self.player.y,
                    self.player.w as u32,
                    self.player.h as u32,
                );
                if self.bullet.produce(player_pos) {
                    if let Some(ch) = self.group.find_available() {
                        ch.play(self.musics.get("bullet").unwrap(), 1).unwrap();
                    }
                }

                for (plane_pos, _) in self.enemy.planes.iter() {
                    self.canvas
                        .copy(self.images.get("enemy").unwrap(), None, Some(*plane_pos))
                        .unwrap();
                }
                self.canvas
                    .copy(self.images.get("player").unwrap(), None, Some(player_pos))
                    .unwrap();
                for (pos, _) in self.bullet.poss.iter() {
                    self.canvas
                        .copy(self.images.get("bullet").unwrap(), None, Some(*pos))
                        .unwrap();
                }

                let mut collide = false;
                for (plane_pos, plane_live) in self.enemy.planes.iter_mut() {
                    for (bul_pos, bul_live) in self.bullet.poss.iter_mut() {
                        if *plane_live
                            && *bul_live
                            && bul_pos.x >= plane_pos.x
                            && bul_pos.x <= plane_pos.x + plane_pos.width() as i32
                            && bul_pos.y >= plane_pos.y
                            && bul_pos.y <= plane_pos.y + plane_pos.height() as i32
                        {
                            *plane_live = false;
                            *bul_live = false;
                            self.score += 1;
                            if let Some(ch) = self.group.find_available() {
                                ch.play(self.musics.get("enemy_down").unwrap(), 1).unwrap();
                            }
                        }
                    }
                    if *plane_live
                        && plane_pos.x >= player_pos.x
                        && plane_pos.x <= player_pos.x + player_pos.width() as i32
                        && plane_pos.y >= player_pos.y
                        && plane_pos.y <= player_pos.y + player_pos.height() as i32
                    {
                        collide = true;
                        break;
                    }
                }
                if collide {
                    self.state = State::Over;
                    self.enemy.planes.clear();
                    self.bullet.poss.clear();
                    if let Some(ch) = self.group.find_available() {
                        ch.play(self.musics.get("me_down").unwrap(), 0).unwrap();
                    }
                }
            }
            State::Over => {
                self.canvas
                    .copy(self.images.get("over").unwrap(), None, None)
                    .unwrap();
            }
        }
    }

    pub fn run(&mut self) {
        let mut event_pump = self.sdl.event_pump().unwrap();
        let ttf_context = sdl2::ttf::init().unwrap();
        let font = ttf_context
            .load_font("./res/SourceCodePro-Bold.ttf", 16)
            .unwrap();
        let txt_creator = self.canvas.texture_creator();

        loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => return,
                    Event::KeyDown {
                        keycode: Some(Keycode::Space),
                        ..
                    } => {
                        if !matches!(self.state, State::Playing) {
                            self.state = State::Playing;
                            self.score = 0;
                        }
                    }
                    Event::MouseMotion { x, y, .. } => self.player.move_to(x, y),
                    _ => (),
                }
            }
            self.render();

            let s = format!("SCORE:{}", self.score);
            let sb = s.as_bytes();
            let txt = font
                .render_latin1(sb)
                .blended(Color::RGB(196, 96, 96))
                .unwrap()
                .as_texture(&txt_creator)
                .unwrap();
            let (w, h) = font.size_of_latin1(sb).unwrap();
            let rect = Rect::new(8, 8, w, h);
            self.canvas.copy(&txt, None, Some(rect)).unwrap();
            self.canvas.present();
        }
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        close_audio();
    }
}
