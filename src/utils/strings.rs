//! This module contains very boring logic. You probably don't want to read it.

use std::time::Instant;

use rand::Rng;

const COOLDOWN_MS: u128 = 850;

fn get_tier(count: u32) -> Option<u8> {
    match count {
        8..=39 => Some(1),
        40..=64 => Some(2),
        65..=99 => Some(3),
        100.. => Some(4),
        _ => None,
    }
}

pub fn choose_string(
    count: u32,
    last_tier: Option<u8>,
    last_change: Option<Instant>,
) -> Option<(&'static str, u8)> {
    if let Some(last) = last_change {
        if last.elapsed().as_millis() < COOLDOWN_MS {
            return None;
        }
    }

    let current_tier = get_tier(count)?;

    let activation_chance = match last_tier {
        Some(last_tier) if current_tier <= last_tier => 0.12,
        _ => match current_tier {
            1 => 0.7,
            2 => 0.5,
            3 => 0.3,
            _ => 0.2,
        },
    };

    let mut rng = rand::rng();
    if rng.random_range(0.0..1.0) < activation_chance {
        let strings = match current_tier {
            1 => STRING_TIER_1,
            2 => STRING_TIER_2,
            3 => STRING_TIER_3,
            _ => STRING_TIER_4,
        };
        Some((strings[rng.random_range(0..strings.len())], current_tier))
    } else {
        None
    }
}

const STRING_TIER_1: &[&str] = &[
    "wooOOOOOooosh",
    "Spinning into the void...",
    "Wheeeeee!",
    "Round and round we go",
];

const STRING_TIER_2: &[&str] = &[
    "You found the spinny thing!",
    "Is this what you wanted?",
    "Spin to win!",
    "I'm getting dizzy...",
    "Still spinning?",
];

const STRING_TIER_3: &[&str] = &[
    "Okay you really like clicking this",
    "Still going, huh?",
    "I admire your dedication",
    "This is getting out of hand",
    "You've unlocked: persistence",
    "Achievement: Fidgeter",
];

const STRING_TIER_4: &[&str] = &[
    "You've clicked this so many times...",
    "Don't you have a conversation to start?",
    "I'm just a logo, you know",
    "At this point we're friends",
    "The logo appreciates your attention",
    "Employee of the month: You",
    "This is your life now",
];
