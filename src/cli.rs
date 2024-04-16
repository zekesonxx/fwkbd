use clap::{Parser, ValueEnum};


#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all="verbatim")]
pub enum KeyframeFunction {
    EaseIn,
    EaseInCubic,
    EaseInOut,
    EaseInOutCubic,
    EaseInOutQuad,
    EaseInOutQuart,
    EaseInOutQuint,
    EaseInQuad,
    EaseInQuart,
    EaseInQuint,
    EaseOut,
    EaseOutCubic,
    EaseOutQuad,
    EaseOutQuart,
    EaseOutQuint,
    Linear,
}

impl keyframe::EasingFunction for KeyframeFunction {
    fn y(&self, x: f64) -> f64 {
        match *self {
            Self::EaseIn => (keyframe::functions::EaseIn{}).y(x),
            Self::EaseInCubic => (keyframe::functions::EaseInCubic{}).y(x),
            Self::EaseInOut => (keyframe::functions::EaseInOut{}).y(x),
            Self::EaseInOutCubic => (keyframe::functions::EaseInOutCubic{}).y(x),
            Self::EaseInOutQuad => (keyframe::functions::EaseInOutQuad{}).y(x),
            Self::EaseInOutQuart => (keyframe::functions::EaseInOutQuart{}).y(x),
            Self::EaseInOutQuint => (keyframe::functions::EaseInOutQuint{}).y(x),
            Self::EaseInQuad => (keyframe::functions::EaseInQuad{}).y(x),
            Self::EaseInQuart => (keyframe::functions::EaseInQuart{}).y(x),
            Self::EaseInQuint => (keyframe::functions::EaseInQuint{}).y(x),
            Self::EaseOut => (keyframe::functions::EaseOut{}).y(x),
            Self::EaseOutCubic => (keyframe::functions::EaseOutCubic{}).y(x),
            Self::EaseOutQuad => (keyframe::functions::EaseOutQuad{}).y(x),
            Self::EaseOutQuart => (keyframe::functions::EaseOutQuart{}).y(x),
            Self::EaseOutQuint => (keyframe::functions::EaseOutQuint{}).y(x),
            Self::Linear => (keyframe::functions::Linear{}).y(x),
        }
    }
}

/// Unofficial Framework Laptop keyboard backlight fade in/out daemon
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(after_help="Easing curves accept any curve name from the keyframes crate:\nhttps://docs.rs/keyframe/latest/keyframe/functions/index.html")]
pub struct Args {
    /// Seconds until the keyboard backlight times out
    #[arg(short, long, default_value_t = 5.0)]
    pub timeout: f32,

    /// Max brightness setting
    #[arg(short, long, default_value_t = 100)]
    #[arg(value_parser = clap::value_parser!(u8).range(1..=100))]
    pub brightness: u8,

    /// Disable the userspace led, even if the module is present
    #[arg(long, default_value_t = false)]
    pub no_uleds: bool,
    
    /// How long, in seconds, to fade the keyboard backlight in
    #[arg(short='i', long, default_value_t = 0.2)]
    pub fade_in: f32,

    /// How long, in seconds, to fade the keyboard backlight out
    #[arg(short='o', long, default_value_t = 2.0)]
    pub fade_out: f32,

    /// Animation curve for fading in
    #[arg(long, value_enum, hide_possible_values=true, default_value_t = KeyframeFunction::EaseInQuad)]
    pub ease_in: KeyframeFunction,

    /// Animation curve for fading out
    #[arg(long, value_enum, hide_possible_values=true, default_value_t = KeyframeFunction::EaseOut)]
    pub ease_out: KeyframeFunction,
}