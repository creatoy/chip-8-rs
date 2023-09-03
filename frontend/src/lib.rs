use chip;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [Self::Channel]) {
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

pub struct Display {
    canvas: Canvas<Window>,
    audio: AudioDevice<SquareWave>,
    event_pump: sdl2::EventPump,
    pixel_scale: u32,
}

impl Display {
    pub fn new(pixel_scale: u32) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let audio_subsystem = sdl_context.audio().unwrap();
        let audio = audio_subsystem
            .open_playback(
                None,
                &AudioSpecDesired {
                    freq: Some(44100),
                    channels: Some(1),
                    samples: None,
                },
                |spec| SquareWave {
                    phase_inc: 440.0 / spec.freq as f32,
                    phase: 0.0,
                    volume: 0.25,
                },
            )
            .unwrap();

        let window = video_subsystem
            .window(
                "CHIP-8 Emulator",
                chip::DISP_WIDTH as u32 * pixel_scale,
                chip::DISP_HEIGHT as u32 * pixel_scale,
            )
            .position_centered()
            .build()
            .unwrap();

        let canvas = window.into_canvas().build().unwrap();

        let event_pump = sdl_context.event_pump().unwrap();

        Self {
            canvas,
            audio,
            event_pump,
            pixel_scale,
        }
    }

    fn draw(&mut self, chip: &chip::Chip) {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        let fb = chip.framebuffer();
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        for (i, pixel) in fb.iter().enumerate() {
            if *pixel {
                let rect = Rect::new(
                    (i % chip::DISP_WIDTH) as i32 * self.pixel_scale as i32,
                    (i / chip::DISP_WIDTH) as i32 * self.pixel_scale as i32,
                    self.pixel_scale,
                    self.pixel_scale,
                );
                self.canvas.fill_rect(rect).unwrap();
            }
        }
        self.canvas.present();
    }

    pub fn update(&mut self, chip: &mut chip::Chip) -> Result<(), chip::Exception> {
        match self.event_pump.poll_event() {
            Some(event) => match event {
                Event::Quit { .. } => return Err(chip::Exception::Halt(0)),
                Event::AppTerminating { timestamp } => return Err(chip::Exception::Halt(0)),
                Event::KeyDown {
                    keycode: Some(k), ..
                } => match k {
                    Keycode::Escape => return Err(chip::Exception::Halt(0)),
                    _ => {
                        if let Some(key) = Self::keycode_to_keypad(k) {
                            // println!("Key pressed: {}", key);
                            chip.set_keypad(key, true);
                        }
                    }
                },
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Some(key) = Self::keycode_to_keypad(k) {
                        // println!("Key released: {}", key);
                        chip.set_keypad(key, false);
                    }
                }
                _ => (),
            },
            None => (),
        }

        chip.tick()?;

        if chip.tone() {
            self.audio.resume();
        } else {
            self.audio.pause();
        }
        self.draw(chip);

        Ok(())
    }

    fn keycode_to_keypad(keycode: Keycode) -> Option<u8> {
        match keycode {
            Keycode::Num1 => Some(1u8),
            Keycode::Num2 => Some(2u8),
            Keycode::Num3 => Some(3u8),
            Keycode::Num4 => Some(0xCu8),
            Keycode::Q => Some(4u8),
            Keycode::W => Some(5u8),
            Keycode::E => Some(6u8),
            Keycode::R => Some(0xDu8),
            Keycode::A => Some(7u8),
            Keycode::S => Some(8u8),
            Keycode::D => Some(9u8),
            Keycode::F => Some(0xEu8),
            Keycode::Z => Some(0xAu8),
            Keycode::X => Some(0u8),
            Keycode::C => Some(0xBu8),
            Keycode::V => Some(0xFu8),
            _ => None,
        }
    }
}
