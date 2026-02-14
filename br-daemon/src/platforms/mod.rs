pub mod cookie_utils;
pub mod kick;
pub mod traits;
pub mod twitch;
pub mod youtube;

#[cfg(any(test, feature = "test-utils"))]
pub mod mock;

pub use cookie_utils::{
    delete_youtube_cookies, get_youtube_cookie_path, save_youtube_cookies, validate_cookie_file,
    youtube_cookies_exist, CookieError,
};
pub use kick::KickPlatform;
pub use traits::*;
pub use twitch::TwitchPlatform;
pub use youtube::{is_bun_available, is_ytdlp_available, YoutubePlatform};

#[cfg(any(test, feature = "test-utils"))]
pub use mock::{MockChannelConfig, MockError, MockPlatform, MockPlatformBuilder};
