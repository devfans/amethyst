//! TODO: Rewrite for new renderer.

extern crate amethyst;
extern crate serde_json;
extern crate futures;
extern crate tokio;
extern crate tokio_codec;
extern crate bytes;
extern crate tokio_io;
extern crate winit;
extern crate moba_proto;

mod audio;
mod bundle;
mod pong;
mod systems;

use winit::VirtualKeyCode;

use amethyst::audio::AudioBundle;
use amethyst::core::frame_limiter::FrameRateLimitStrategy;
use amethyst::core::transform::TransformBundle;
use amethyst::ecs::prelude::{Component, DenseVecStorage};
use amethyst::input::{InputBundle, InputEvent};
use amethyst::prelude::*;
use amethyst::renderer::{DisplayConfig, DrawSprite, Pipeline, RenderBundle, Stage};
use amethyst::ui::{DrawUi, UiBundle};

use audio::Music;
use bundle::PongBundle;
use std::time::Duration;

use serde_json::Value;
use moba_proto::Service;
use moba_proto::Message;

use std::fs;
use std::env;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use futures::{Stream, future};
use futures::sync::mpsc::{unbounded};

const ARENA_HEIGHT: f32 = 100.0;
const ARENA_WIDTH: f32 = 100.0;
const PADDLE_HEIGHT: f32 = 16.0;
const PADDLE_WIDTH: f32 = 4.0;
const PADDLE_VELOCITY: f32 = 75.0;

const BALL_VELOCITY_X: f32 = 75.0;
const BALL_VELOCITY_Y: f32 = 50.0;
const BALL_RADIUS: f32 = 2.0;

const SPRITESHEET_SIZE: (f32, f32) = (8.0, 16.0);

const AUDIO_MUSIC: &'static [&'static str] = &[
    "audio/Computer_Music_All-Stars_-_Wheres_My_Jetpack.ogg",
    "audio/Computer_Music_All-Stars_-_Albatross_v2.ogg",
];
const AUDIO_BOUNCE: &'static str = "audio/bounce.ogg";
const AUDIO_SCORE: &'static str = "audio/score.ogg";

fn main() -> amethyst::Result<()> {
    let mut conf_path = None;

    for arg in env::args().skip(1) {
        if arg.starts_with("-c") {
            conf_path = Some(arg.split_at(14).1.to_string());
        }
    }

    let conf = if let Some(path) = conf_path {
        path
    } else {
        "./config.json".to_string()
    };
    println!("Loading configuration from {}", conf);

    let conf_file = fs::File::open(conf).expect("Failed to read config file");
    let config: Value= serde_json::from_reader(conf_file).expect("Failed to parse config file");
    let (sink, stream) = unbounded();
    let (tx, rx) = mpsc::channel();
    let sink = Arc::new(Mutex::new(sink));
    let tx = Arc::new(Mutex::new(tx));
    let service = Service::new(config);
    let service_ref = service.clone();
    let listen_addr = service.addr;
    println!("Client is connecting to {}", listen_addr);
    let mut rt = tokio::runtime::Builder::new().build().unwrap();
    rt.spawn(stream.for_each(move |msg| {
        match msg {
            Message::DataFrame { code, .. } => {
                let tx = tx.lock().unwrap();
                let event: Option<InputEvent<String>> = match code {
                    1 => Some(InputEvent::KeyPressed { key_code: VirtualKeyCode::S, scancode: 1 }),
                    5 => Some(InputEvent::KeyReleased { key_code: VirtualKeyCode::S, scancode: 1 }),
                    13 => Some(InputEvent::KeyPressed { key_code: VirtualKeyCode::W, scancode: 13 }),
                    6 => Some(InputEvent::KeyReleased { key_code: VirtualKeyCode::W, scancode: 13 }),
                    125 => Some(InputEvent::KeyPressed { key_code: VirtualKeyCode::Down, scancode: 125 }),
                    7 => Some(InputEvent::KeyReleased { key_code: VirtualKeyCode::Down, scancode: 125 }),
                    126 => Some(InputEvent::KeyPressed { key_code: VirtualKeyCode::Up, scancode: 126 }),
                    8 => Some(InputEvent::KeyReleased { key_code: VirtualKeyCode::Up, scancode: 126 }),
                    _ => None,
                };
                if let Some(event) = event {
                    tx.send(vec![event]).unwrap();
                }
            },
            _ => {}
        }
        Ok(())
    }));

    rt.spawn(future::lazy(move || -> Result<(), ()> {
        service_ref.connect(sink);
        Ok(())
    }));

    amethyst::start_logger(Default::default());

    use pong::Pong;

    let display_config_path = format!(
        "{}/examples/pong/resources/display.ron",
        env!("CARGO_MANIFEST_DIR")
    );
    let config = DisplayConfig::load(&display_config_path);

    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
            .with_pass(DrawSprite::new())
            .with_pass(DrawUi::new()),
    );

    let key_bindings_path = {
        if cfg!(feature = "sdl_controller") {
            format!(
                "{}/examples/pong/resources/input_controller.ron",
                env!("CARGO_MANIFEST_DIR")
            )
        } else {
            format!(
                "{}/examples/pong/resources/input.ron",
                env!("CARGO_MANIFEST_DIR")
            )
        }
    };

    let assets_dir = format!("{}/examples/assets/", env!("CARGO_MANIFEST_DIR"));

    let game_data = GameDataBuilder::default()
        .with_bundle(
            InputBundle::<String, String>::new().with_bindings_from_file(&key_bindings_path)?,
        )?
        .with_bundle(PongBundle)?
        .with_bundle(RenderBundle::new(pipe, Some(config)).with_sprite_sheet_processor())?
        .with_bundle(TransformBundle::new().with_dep(&["ball_system", "paddle_system"]))?
        .with_bundle(AudioBundle::new(|music: &mut Music| music.music.next()))?
        .with_bundle(UiBundle::<String, String>::new())?;
    let rx = Some(Arc::new(Mutex::new(rx)));
    let mut game = Application::build(assets_dir, Pong)?
        .with_resource(rx)
        .with_frame_limit(
            FrameRateLimitStrategy::SleepAndYield(Duration::from_millis(20)),
            10,
        )
        .build(game_data)?;
    game.run();
    Ok(())
}

pub struct Ball {
    pub velocity: [f32; 2],
    pub radius: f32,
}

impl Component for Ball {
    type Storage = DenseVecStorage<Self>;
}

#[derive(PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub struct Paddle {
    pub velocity: f32,
    pub side: Side,
    pub width: f32,
    pub height: f32,
}

impl Paddle {
    pub fn new(side: Side) -> Paddle {
        Paddle {
            velocity: 1.0,
            side: side,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl Component for Paddle {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Default)]
pub struct ScoreBoard {
    score_left: i32,
    score_right: i32,
}

impl ScoreBoard {
    pub fn new() -> ScoreBoard {
        ScoreBoard {
            score_left: 0,
            score_right: 0,
        }
    }
}
