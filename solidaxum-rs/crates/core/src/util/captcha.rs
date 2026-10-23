use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageEncoder;
use rand::RngExt;

/// 生成数学运算验证码，返回 (展示表达式, 答案)。
pub fn generate_math_captcha() -> (String, String) {
    let mut rng = rand::rng();
    let a: i32 = rng.random_range(1..=10);
    let b: i32 = rng.random_range(1..=10);
    let ops = ['+', '-', '*'];
    let op = ops[rng.random_range(0..3)];

    let result = match op {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        _ => 0,
    };

    (format!("{}{}{}=", a, op, b), result.to_string())
}

/// 生成字符验证码，返回 (code, code)。仅使用大写字母+数字，便于渲染与校验。
pub fn generate_char_captcha() -> (String, String) {
    let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
    let mut rng = rand::rng();
    let code: String = (0..4)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect();
    (code.clone(), code)
}

pub fn generate_uuid() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

/// 将文本渲染为 PNG 图片并返回 **裸 base64**（不含 `data:...;base64,` 前缀）。
///
/// 前端（原版 Vue）会自行拼接 `data:image/gif;base64,` 前缀，故此处只返回 base64 正文；
/// 浏览器对 `<img>` 的 data URL 会做内容嗅探，PNG 字节在 `image/gif` 声明下可正常显示。
pub fn render_png_base64(text: &str) -> String {
    const SCALE: u32 = 4;
    let gw = 5 * SCALE;
    let gh = 7 * SCALE;
    let gap = SCALE;
    let pad_x: u32 = 12;
    let pad_y: u32 = 16;

    let count = text.chars().count().max(1) as u32;
    let w = pad_x * 2 + count * (gw + gap);
    let h = pad_y * 2 + gh;

    // 白底不透明
    let mut px = vec![255u8; (w * h * 4) as usize];

    // 绘制文本
    let fg = [31u8, 31, 130, 255];
    for (i, ch) in text.chars().enumerate() {
        let pat = glyph_pattern(ch);
        for (row, line) in pat.lines().enumerate() {
            for (col, c) in line.chars().enumerate() {
                if c == '#' {
                    let x0 =
                        pad_x + (i as u32) * (gw + gap) + (col as u32) * SCALE;
                    let y0 = pad_y + (row as u32) * SCALE;
                    put_block(&mut px, w, h, x0, y0, SCALE, fg);
                }
            }
        }
    }

    // 随机干扰线，增强验证码辨识难度
    let mut rng = rand::rng();
    for _ in 0..4 {
        let col = [
            rng.random_range(60..180),
            rng.random_range(60..180),
            rng.random_range(100..200),
            255,
        ];
        let x0 = rng.random_range(0..w as i32);
        let y0 = rng.random_range(0..h as i32);
        let x1 = rng.random_range(0..w as i32);
        let y1 = rng.random_range(0..h as i32);
        draw_line(&mut px, w, h, x0, y0, x1, y1, col);
    }

    // PNG 编码
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder
        .write_image(&px, w, h, image::ExtendedColorType::Rgba8)
        .expect("PNG 编码失败");
    STANDARD.encode(&buf)
}

/// 5x7 位图字体，返回 7 行（每行 5 字符），'#' 代表像素点。
/// 仅覆盖验证码所需字符：数字、运算符、大写字母。
fn glyph_pattern(c: char) -> &'static str {
    match c {
        '0' => " ### \n#   #\n#  ##\n# # #\n##  #\n#   #\n ### ",
        '1' => "  #  \n ##  \n  #  \n  #  \n  #  \n  #  \n ### ",
        '2' => " ### \n#   #\n    #\n   # \n  #  \n #   \n#####",
        '3' => " ### \n#   #\n    #\n  ## \n    #\n#   #\n ### ",
        '4' => "   # \n  ## \n # # \n#  # \n#####\n   # \n   # ",
        '5' => "#####\n#    \n#### \n    #\n    #\n#   #\n ### ",
        '6' => " ### \n#    \n#    \n#### \n#   #\n#   #\n ### ",
        '7' => "#####\n    #\n   # \n  #  \n  #  \n #   \n#    ",
        '8' => " ### \n#   #\n#   #\n ### \n#   #\n#   #\n ### ",
        '9' => " ### \n#   #\n#   #\n ####\n    #\n    #\n ### ",
        '+' => "     \n  #  \n  #  \n#####\n  #  \n  #  \n     ",
        '-' => "     \n     \n     \n#####\n     \n     \n     ",
        '*' => "     \n  #  \n# # #\n ### \n# # #\n  #  \n     ",
        '=' => "     \n     \n#####\n     \n#####\n     \n     ",
        ' ' => "     \n     \n     \n     \n     \n     \n     ",
        'A' => "  #  \n # # \n#   #\n#   #\n#####\n#   #\n#   #",
        'B' => "#### \n#   #\n#   #\n#### \n#   #\n#   #\n#### ",
        'C' => " ### \n#   #\n#    \n#    \n#    \n#   #\n ### ",
        'D' => "###  \n#  # \n#   #\n#   #\n#   #\n#  # \n###  ",
        'E' => "#####\n#    \n#    \n#### \n#    \n#    \n#####",
        'F' => "#####\n#    \n#    \n#### \n#    \n#    \n#    ",
        'G' => " ### \n#   #\n#    \n# ###\n#   #\n#   #\n ### ",
        'H' => "#   #\n#   #\n#   #\n#####\n#   #\n#   #\n#   #",
        'I' => " ### \n  #  \n  #  \n  #  \n  #  \n  #  \n ### ",
        'J' => "  ###\n   # \n   # \n   # \n#  # \n#  # \n ##  ",
        'K' => "#   #\n#  # \n##   \n#    \n##   \n#  # \n#   #",
        'L' => "#    \n#    \n#    \n#    \n#    \n#    \n#####",
        'M' => "#   #\n## ##\n# # #\n# # #\n#   #\n#   #\n#   #",
        'N' => "#   #\n##  #\n# # #\n# # #\n#  ##\n#   #\n#   #",
        'O' => " ### \n#   #\n#   #\n#   #\n#   #\n#   #\n ### ",
        'P' => "#### \n#   #\n#   #\n#### \n#    \n#    \n#    ",
        'Q' => " ### \n#   #\n#   #\n#   #\n# # #\n#  # \n ## #",
        'R' => "#### \n#   #\n#   #\n#### \n##   \n#  # \n#   #",
        'S' => " ####\n#    \n#    \n ### \n    #\n    #\n#### ",
        'T' => "#####\n  #  \n  #  \n  #  \n  #  \n  #  \n  #  ",
        'U' => "#   #\n#   #\n#   #\n#   #\n#   #\n#   #\n ### ",
        'V' => "#   #\n#   #\n#   #\n#   #\n#   #\n # # \n  #  ",
        'W' => "#   #\n#   #\n#   #\n# # #\n# # #\n## ##\n#   #",
        'X' => "#   #\n#   #\n # # \n  #  \n # # \n#   #\n#   #",
        'Y' => "#   #\n#   #\n # # \n  #  \n  #  \n  #  \n  #  ",
        'Z' => "#####\n    #\n   # \n  #  \n #   \n#    \n#####",
        // 未知字符渲染为实心方块，保证布局不塌陷
        _ => "#####\n#   #\n#   #\n#   #\n#   #\n#   #\n#####",
    }
}

fn put_block(
    px: &mut [u8],
    w: u32,
    h: u32,
    x0: u32,
    y0: u32,
    scale: u32,
    col: [u8; 4],
) {
    for dy in 0..scale {
        for dx in 0..scale {
            let x = x0 + dx;
            let y = y0 + dy;
            if x < w && y < h {
                let idx = ((y * w + x) * 4) as usize;
                px[idx..idx + 4].copy_from_slice(&col);
            }
        }
    }
}

/// Bresenham 画线（用于干扰线）。
#[allow(clippy::too_many_arguments)]
fn draw_line(
    px: &mut [u8],
    w: u32,
    h: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    col: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
            let idx = ((y as u32 * w + x as u32) * 4) as usize;
            px[idx..idx + 4].copy_from_slice(&col);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_captcha_answer_is_numeric() {
        let (expr, ans) = generate_math_captcha();
        assert!(expr.ends_with('='));
        ans.parse::<i32>().expect("答案应为整数");
    }

    #[test]
    fn char_captcha_is_uppercase_alphanumeric() {
        let (code, ans) = generate_char_captcha();
        assert_eq!(code, ans);
        assert!(
            code.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
        assert_eq!(code.len(), 4);
    }

    #[test]
    fn render_png_returns_raw_base64_without_prefix() {
        let b64 = render_png_base64("3+5=");
        assert!(!b64.starts_with("data:"), "不应包含 data: 前缀");
        let bytes = STANDARD.decode(&b64).expect("应为合法 base64");
        // PNG 魔数头
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn render_png_covers_letters_and_operators() {
        for s in ["A", "Z", "9", "+", "-", "*", "=", "RY4K="] {
            let b64 = render_png_base64(s);
            let bytes = STANDARD.decode(&b64).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{s} 编码失败");
        }
    }
}
