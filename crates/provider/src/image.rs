//! The images in a message's content: each one found at any depth, read,
//! and shrunk to a size the API accepts in any request.

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::imageops::FilterType;
use image::{ImageFormat, ImageReader};
use serde_json::Value;

/// The longest edge an image is sent with. The API scales a longer one down
/// to this before the model sees it (2576 px from Claude 4.7, 1568 px
/// before), and refuses an edge over 3000 px once a request holds more than
/// 20 images, so shrinking to it loses nothing the model would have seen and
/// holds for a request with any number of images.
const MAX_EDGE: u32 = 2576;

/// `content` (a message's JSON blocks) with every base64 image whose longer
/// edge is over [`MAX_EDGE`] scaled down to it, at any depth (a tool
/// result's images too); the same bytes when there is none. The same content
/// always comes out the same, so its cached prefix stays cached.
///
/// An image that cannot be read is left as it is, for the API to judge.
pub fn shrunk(content: Vec<u8>) -> Vec<u8> {
    if !content.windows(7).any(|w| w == b"\"image\"") {
        return content;
    }
    let Ok(mut blocks) = serde_json::from_slice::<Value>(&content) else {
        return content;
    };
    if !shrink_all(&mut blocks) {
        return content;
    }
    serde_json::to_vec(&blocks).expect("a JSON value serialises")
}

/// Shrinks every image in `value`; whether any was.
fn shrink_all(value: &mut Value) -> bool {
    let mut any = false;
    each_image(value, &mut |source| any |= shrink(source));
    any
}

/// Calls `f` with the `source` of every image block in `value` (a message's
/// JSON blocks), at any depth (a tool result's images too), always in the
/// same order for the same value: the `n`th call is its `n`th image.
pub fn each_image(value: &mut Value, f: &mut impl FnMut(&mut Value)) {
    match value {
        Value::Object(block) if block.get("type").and_then(Value::as_str) == Some("image") => {
            if let Some(source) = block.get_mut("source") {
                f(source);
            }
        }
        Value::Object(members) => members.values_mut().for_each(|v| each_image(v, f)),
        Value::Array(items) => items.iter_mut().for_each(|v| each_image(v, f)),
        _ => {}
    }
}

/// The image a base64 `source` holds, as bytes; none for any other source.
pub fn decoded(source: &Value) -> Option<Vec<u8>> {
    STANDARD.decode(source.get("data")?.as_str()?).ok()
}

/// An image's width and height in pixels, from its header.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.into_dimensions().ok()
}

/// Replaces a base64 `source`'s data with the image scaled to fit
/// [`MAX_EDGE`], if it does not already; whether it did. A JPEG stays a
/// JPEG; anything else becomes a PNG.
fn shrink(source: &mut Value) -> bool {
    let Some(bytes) = decoded(source) else {
        return false;
    };
    let Ok(reader) = ImageReader::new(Cursor::new(&bytes)).with_guessed_format() else {
        return false;
    };
    let format = reader.format();
    let Ok((width, height)) = reader.into_dimensions() else {
        return false;
    };
    if width.max(height) <= MAX_EDGE {
        return false;
    }
    let Ok(image) = image::load_from_memory(&bytes) else {
        return false;
    };
    let image = image.resize(MAX_EDGE, MAX_EDGE, FilterType::Triangle);
    let mut out = Cursor::new(Vec::new());
    let (written, media_type) = match format {
        Some(ImageFormat::Jpeg) => (
            image.to_rgb8().write_to(&mut out, ImageFormat::Jpeg),
            "image/jpeg",
        ),
        _ => (image.write_to(&mut out, ImageFormat::Png), "image/png"),
    };
    if written.is_err() {
        return false;
    }
    source["data"] = STANDARD.encode(out.into_inner()).into();
    source["media_type"] = media_type.into();
    true
}
