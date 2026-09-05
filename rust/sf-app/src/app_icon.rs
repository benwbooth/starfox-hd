//! Shared desktop identity and embedded window artwork (no runtime asset path).

use sdl3::{iostream::IOStream, surface::Surface, video::Window};

pub const APPLICATION_ID: &str = "io.github.benwbooth.starfox-hd";
const APPLICATION_ID_HINT: &str = "SDL_APP_ID";
const ICON_BYTES: &[u8] = include_bytes!("../assets/starfox-hd.bmp");

/// Must run before SDL creates a window. Wayland shells resolve this ID to
/// the installed desktop entry; X11 also uses it as the window class.
pub fn configure_identity() {
    sdl3::hint::set(APPLICATION_ID_HINT, APPLICATION_ID);
}

fn decode_icon() -> Result<Surface<'static>, sdl3::Error> {
    let mut stream = IOStream::from_bytes(ICON_BYTES)?;
    Surface::load_bmp_rw(&mut stream)
}

pub fn apply(window: &mut Window) {
    match decode_icon() {
        Ok(icon) => {
            // Some Wayland compositors only support the desktop-entry icon.
            // SDL copies the pixels, so the decoded surface can be dropped.
            window.set_icon(icon);
        }
        Err(error) => eprintln!("Could not decode application icon: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl3::pixels::PixelFormat;

    #[test]
    fn embedded_icon_decodes_with_transparency_and_visible_artwork() {
        const ICON_SIDE: u32 = 256;
        const CHANNEL_COUNT: usize = 4;
        const ALPHA_CHANNEL: usize = CHANNEL_COUNT - 1;
        let icon = decode_icon().expect("embedded BMP must decode without a working directory");
        assert_eq!((icon.width(), icon.height()), (ICON_SIDE, ICON_SIDE));
        let rgba = icon.convert_format(PixelFormat::RGBA32).unwrap();
        rgba.with_lock(|pixels| {
            assert_eq!(pixels[ALPHA_CHANNEL], 0, "corner must be transparent");
            assert!(pixels
                .chunks_exact(CHANNEL_COUNT)
                .any(|pixel| pixel[ALPHA_CHANNEL] == u8::MAX));
        });
    }

    #[test]
    fn desktop_entry_matches_the_window_identity() {
        let entry = include_str!("../assets/io.github.benwbooth.starfox-hd.desktop");
        assert!(entry
            .lines()
            .any(|line| line == format!("Icon={APPLICATION_ID}")));
        assert!(entry
            .lines()
            .any(|line| line == format!("StartupWMClass={APPLICATION_ID}")));
    }
}
