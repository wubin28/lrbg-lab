#![deny(unsafe_code)]
#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_hal::digital::v2::InputPin;
use microbit::board::Board;
use microbit::display::blocking::Display;
use microbit::hal::Timer;
use panic_probe as _;
use rtt_target::rtt_init;

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

static SMILEY: [[u8; 5]; 5] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 0, 0, 0, 0],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
];

static PATTERNS: [[[u8; 5]; 5]; 10] = [
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
    let button_a = board.buttons.button_a;

    let mut game_state = GameState::ShowingSmiley;
    let mut display_buffer = [[0u8; 5]; 5];
    let mut current_pattern = 0;

    let seed = timer.read();
    let mut rng = XorShiftRng::new(seed);

    let mut last_button_state = button_a.is_low().unwrap();

    loop {
        match game_state {
            GameState::ShowingSmiley => {
                copy_pattern_to_buffer(&SMILEY, &mut display_buffer);
                display.show(&mut timer, display_buffer, 100);

                let current_button_state = button_a.is_low().unwrap();

                if current_button_state && !last_button_state {
                    clear_buffer(&mut display_buffer);
                    display.show(&mut timer, display_buffer, 100);
                    game_state = GameState::ShowingPatterns;
                }

                last_button_state = current_button_state;
            }

            GameState::ShowingPatterns => {
                current_pattern = rng.next_range(10);
                copy_pattern_to_buffer(&PATTERNS[current_pattern], &mut display_buffer);

                // Split the 1000ms delay into multiple shorter delays,
                // check the button status each time
                let mut elapsed = 0;
                while elapsed < 1000 {
                    // Refresh the display every 50ms
                    display.show(&mut timer, display_buffer, 50);

                    let current_button_state = button_a.is_low().unwrap();

                    if current_button_state && !last_button_state {
                        writeln!(channel, "Selected pattern: {}", current_pattern).ok();
                        game_state = GameState::ShowingSmiley;
                        break;
                    }

                    last_button_state = current_button_state;

                    elapsed += 50;
                }
            }
        }
    }
}

fn copy_pattern_to_buffer(pattern: &[[u8; 5]; 5], buffer: &mut [[u8; 5]; 5]) {
    for row in 0..5 {
        for col in 0..5 {
            buffer[row][col] = if pattern[row][col] == 1 { 9 } else { 0 };
        }
    }
}

fn clear_buffer(buffer: &mut [[u8; 5]; 5]) {
    for row in 0..5 {
        for col in 0..5 {
            buffer[row][col] = 0;
        }
    }
}
