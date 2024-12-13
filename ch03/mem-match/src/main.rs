#![deny(unsafe_code)]
#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::InputPin;
use microbit::board::Board;
use microbit::display::blocking::Display;
use microbit::hal::gpio::Level;
use microbit::hal::prelude::*;
use microbit::hal::pwm::{Channel, Pwm};
use microbit::hal::time::Hertz;
use microbit::hal::Timer;
use microbit::pac::{PWM0, TIMER0};
use panic_probe as _;
use rtt_target::rtt_init;

const LONG_DELAY_MS: u32 = 1000;
const MEDIUM_DELAY_MS: u32 = 100;
const SHORT_DELAY_MS: u32 = 50;

const LED_MATRIX_DIMENSION: usize = 5;

const PATTERN_NUM: usize = 10;

const BUZZER_FREQ_HZ: u32 = 1000;

const MAX_LED_BRIGHTNESS: u8 = 9;
const MIN_LED_BRIGHTNESS: u8 = 0;

struct XorShiftRng {
    state: u32,
}

impl XorShiftRng {
    fn new(seed: u32) -> Self {
        XorShiftRng {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn next_range(&mut self, range: usize) -> usize {
        (self.next() as usize) % range
    }
}

static SMILEY: [[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 0, 0, 0, 0],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
];

static BIG_SMILEY: [[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1],
    [0, 1, 1, 1, 0],
];

static CRYING_SMILEY: [[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 0, 0, 0, 0],
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
];

static PATTERNS: [[[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION]; PATTERN_NUM] = [
    // 0 Heart shape
    [
        [0, 1, 0, 1, 0],
        [1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
    ],
    // 1 Up arrow
    [
        [0, 0, 1, 0, 0],
        [0, 1, 1, 1, 0],
        [1, 0, 1, 0, 1],
        [0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0],
    ],
    // 2 Solid square
    [
        [0, 0, 1, 0, 0],
        [0, 1, 1, 1, 0],
        [1, 1, 1, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
    ],
    // 3 Hollow square
    [
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    // 4 X shape
    [
        [1, 0, 0, 0, 1],
        [0, 1, 0, 1, 0],
        [0, 0, 1, 0, 0],
        [0, 1, 0, 1, 0],
        [1, 0, 0, 0, 1],
    ],
    // 5 Hollow pointed roof
    [
        [0, 0, 1, 0, 0],
        [0, 1, 0, 1, 0],
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [0, 0, 0, 0, 0],
    ],
    // 6 Circle
    [
        [0, 1, 1, 1, 0],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [0, 1, 1, 1, 0],
    ],
    // 7 Plus sign
    [
        [0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0],
        [1, 1, 1, 1, 1],
        [0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0],
    ],
    // 8 Lightning bolt
    [
        [1, 1, 0, 0, 0],
        [0, 1, 0, 0, 0],
        [0, 1, 1, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 1, 1, 0],
    ],
    // 9 Wave
    [
        [0, 0, 0, 0, 0],
        [1, 0, 1, 0, 1],
        [1, 1, 1, 1, 1],
        [0, 1, 0, 1, 0],
        [0, 0, 0, 0, 0],
    ],
];

#[derive(PartialEq)]
enum GameState {
    ShowingSmiley,
    ShowingPatterns,
    ShowingTargetPattern,
}

fn make_beep(pwm: &mut Pwm<PWM0>, timer: &mut Timer<TIMER0>) {
    pwm.set_period(Hertz(BUZZER_FREQ_HZ));
    pwm.set_duty_on_common(pwm.max_duty() / 2);
    pwm.enable(Channel::C0);
    timer.delay_ms(MEDIUM_DELAY_MS);
    pwm.disable(Channel::C0);
}

#[entry]
fn main() -> ! {
    let mut channels = rtt_init! {
        up: {
            0: {
                size: 1024
                mode: NoBlockTrim
                name: "Terminal"
            }
        }
    };
    let channel = &mut channels.up.0;

    let board = Board::take().unwrap();
    let mut display = Display::new(board.display_pins);
    let mut timer = Timer::new(board.TIMER0);

    // Enable the speaker
    board.pins.p0_02.into_push_pull_output(Level::High);

    // Get the speaker output pin
    let speaker_pin = board.speaker_pin.into_push_pull_output(Level::Low);

    let button_a = board.buttons.button_a;
    let button_b = board.buttons.button_b;

    // Get P0_13 from the board and convert it to push-pull output
    board
        .pins
        .p0_13
        .into_push_pull_output(microbit::hal::gpio::Level::Low);

    // Initialize the PWM (Pulse Width Modulation)
    let mut pwm = Pwm::new(board.PWM0);
    // Degrade the speaker pin to a general purpose pin and set it as a PWM output pin
    pwm.set_output_pin(Channel::C0, speaker_pin.degrade());

    let mut current_state = GameState::ShowingSmiley;
    let mut display_buffer = [[0u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION];
    let mut current_pattern;
    let mut target_pattern = usize::MAX;
    let seed = timer.read();
    let mut rng = XorShiftRng::new(seed);

    let mut was_button_a_pressed = button_a.is_low().unwrap();
    let mut was_button_b_pressed = button_b.is_low().unwrap();

    loop {
        match current_state {
            GameState::ShowingSmiley => {
                copy_pattern_to_buffer(&SMILEY, &mut display_buffer);
                display.show(&mut timer, display_buffer, MEDIUM_DELAY_MS);

                let is_button_a_pressed = button_a.is_low().unwrap();

                if is_button_a_pressed && !was_button_a_pressed {
                    clear_buffer(&mut display_buffer);
                    display.show(&mut timer, display_buffer, MEDIUM_DELAY_MS);

                    make_beep(&mut pwm, &mut timer);
                    timer.delay_ms(MEDIUM_DELAY_MS);
                    make_beep(&mut pwm, &mut timer);

                    current_state = GameState::ShowingTargetPattern;
                }

                was_button_a_pressed = is_button_a_pressed;
            }

            GameState::ShowingTargetPattern => {
                current_pattern = rng.next_range(PATTERNS.len());
                target_pattern = current_pattern;
                writeln!(channel, "Target pattern: {}", current_pattern).ok();
                copy_pattern_to_buffer(&PATTERNS[current_pattern], &mut display_buffer);
                display.show(&mut timer, display_buffer, LONG_DELAY_MS);
                current_state = GameState::ShowingPatterns;
            }

            GameState::ShowingPatterns => {
                current_pattern = rng.next_range(PATTERNS.len());
                copy_pattern_to_buffer(&PATTERNS[current_pattern], &mut display_buffer);

                // Split the 1000ms delay into multiple shorter delays,
                // check the button status each time
                let mut elapsed = 0;
                while elapsed < LONG_DELAY_MS {
                    // Refresh the display every 50ms
                    display.show(&mut timer, display_buffer, SHORT_DELAY_MS);

                    let is_button_a_pressed = button_a.is_low().unwrap();
                    let is_button_b_pressed = button_b.is_low().unwrap();

                    if is_button_a_pressed && !was_button_a_pressed {
                        break;
                    }

                    if is_button_b_pressed && !was_button_b_pressed {
                        make_beep(&mut pwm, &mut timer);
                        writeln!(channel, "Selected pattern: {}", current_pattern).ok();

                        if current_pattern == target_pattern {
                            copy_pattern_to_buffer(&BIG_SMILEY, &mut display_buffer);
                            display.show(&mut timer, display_buffer, LONG_DELAY_MS);
                        } else {
                            copy_pattern_to_buffer(&CRYING_SMILEY, &mut display_buffer);
                            display.show(&mut timer, display_buffer, LONG_DELAY_MS);
                        }

                        current_state = GameState::ShowingSmiley;
                        break;
                    }

                    was_button_a_pressed = is_button_a_pressed;
                    was_button_b_pressed = is_button_b_pressed;

                    elapsed += SHORT_DELAY_MS;
                }
            }
        }
    }
}

fn copy_pattern_to_buffer(pattern: &[[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION], buffer: &mut [[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION]) {
    for row in 0..LED_MATRIX_DIMENSION {
        for col in 0..LED_MATRIX_DIMENSION {
            buffer[row][col] = if pattern[row][col] == 1 { MAX_LED_BRIGHTNESS } else { MIN_LED_BRIGHTNESS };
        }
    }
}

fn clear_buffer(buffer: &mut [[u8; LED_MATRIX_DIMENSION]; LED_MATRIX_DIMENSION]) {
    for row in 0..LED_MATRIX_DIMENSION {
        for col in 0..LED_MATRIX_DIMENSION {
            buffer[row][col] = 0;
        }
    }
}
