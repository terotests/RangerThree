//! An RGB colour (three.js `Color`). Components are kept in the linear
//! working colour space; hex values, CSS strings and names are sRGB and are
//! converted on the way in and out, as three.js does with colour management on.

use ranger::prelude::*;

use super::math_utils::{clamp, euclidean_modulo, lerp};
use super::matrix3::Matrix3;
use super::vector2::js_round;
use super::vector3::Vector3;

/// The colour space a value is given or asked in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorSpace {
    /// Linear sRGB primaries, the working space the components are kept in.
    Linear,
    /// sRGB with its transfer curve: hex, CSS and names.
    Srgb,
}

/// Hue, saturation and lightness, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Default for Color {
    fn default() -> Color {
        Color::new(1.0, 1.0, 1.0)
    }
}

pub fn srgb_to_linear(c: f64) -> f64 {
    if c < 0.04045 {
        c * 0.0773993808
    } else {
        (c * 0.9478672986 + 0.0521327014).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f64) -> f64 {
    if c < 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(0.41666) - 0.055
    }
}

fn hue2rgb(p: f64, q: f64, t: f64) -> f64 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * 6.0 * (2.0 / 3.0 - t);
    }
    p
}

impl Color {
    /// A colour from linear components.
    pub fn new(r: f64, g: f64, b: f64) -> Color {
        Color { r, g, b }
    }

    /// A colour from an sRGB hex value such as `0xff8800`.
    pub fn from_hex(hex: i64) -> Color {
        let mut c = Color::new(1.0, 1.0, 1.0);
        c.set_hex(hex, ColorSpace::Srgb);
        c
    }

    /// A colour from a CSS colour string or name; white when it is not one.
    pub fn from_style(style: &str) -> Color {
        let mut c = Color::new(1.0, 1.0, 1.0);
        c.set_style(style, ColorSpace::Srgb);
        c
    }

    fn to_working(&mut self, space: ColorSpace) {
        if space == ColorSpace::Srgb {
            self.r = srgb_to_linear(self.r);
            self.g = srgb_to_linear(self.g);
            self.b = srgb_to_linear(self.b);
        }
    }

    fn in_space(&self, space: ColorSpace) -> Color {
        if space == ColorSpace::Srgb {
            Color::new(linear_to_srgb(self.r), linear_to_srgb(self.g), linear_to_srgb(self.b))
        } else {
            *self
        }
    }

    pub fn set_scalar(&mut self, s: f64) -> &mut Color {
        self.r = s;
        self.g = s;
        self.b = s;
        self
    }

    pub fn set_hex(&mut self, hex: i64, space: ColorSpace) -> &mut Color {
        let h = hex & 0xffffff;
        self.r = ((h >> 16) & 255) as f64 / 255.0;
        self.g = ((h >> 8) & 255) as f64 / 255.0;
        self.b = (h & 255) as f64 / 255.0;
        self.to_working(space);
        self
    }

    pub fn set_rgb(&mut self, r: f64, g: f64, b: f64, space: ColorSpace) -> &mut Color {
        self.r = r;
        self.g = g;
        self.b = b;
        self.to_working(space);
        self
    }

    pub fn set_hsl(&mut self, h: f64, s: f64, l: f64, space: ColorSpace) -> &mut Color {
        let h = euclidean_modulo(h, 1.0);
        let s = clamp(s, 0.0, 1.0);
        let l = clamp(l, 0.0, 1.0);
        if s == 0.0 {
            self.r = l;
            self.g = l;
            self.b = l;
        } else {
            let p = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s };
            let q = 2.0 * l - p;
            self.r = hue2rgb(q, p, h + 1.0 / 3.0);
            self.g = hue2rgb(q, p, h);
            self.b = hue2rgb(q, p, h - 1.0 / 3.0);
        }
        self.to_working(space);
        self
    }

    /// Reads `rgb()`, `rgba()`, `hsl()`, `hsla()`, `#rgb`, `#rrggbb` and the
    /// CSS colour names. Answers false (and leaves the colour) otherwise.
    pub fn set_style(&mut self, style: &str, space: ColorSpace) -> bool {
        let s = style.trim().to_lowercase();
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        if n > 0 && chars[0] == '#' {
            let mut digits: Vec<i64> = Vec::new();
            for i in 1..n {
                let d = hex_digit(chars[i]);
                if d < 0 {
                    return false;
                }
                digits.push(d);
            }
            if digits.len() == 3 {
                self.r = (digits[0] * 17) as f64 / 255.0;
                self.g = (digits[1] * 17) as f64 / 255.0;
                self.b = (digits[2] * 17) as f64 / 255.0;
                self.to_working(space);
                return true;
            }
            if digits.len() == 6 {
                let mut hex: i64 = 0;
                for d in digits.iter() {
                    hex = hex * 16 + *d;
                }
                self.set_hex(hex, space);
                return true;
            }
            return false;
        }
        let mut open = n;
        for i in 0..n {
            if chars[i] == '(' {
                open = i;
                break;
            }
        }
        if open < n && chars[n - 1] == ')' {
            let mut name = String::new();
            for i in 0..open {
                name.push(chars[i]);
            }
            let mut inner = String::new();
            for i in (open + 1)..(n - 1) {
                let c = chars[i];
                if c == ',' || c == '/' {
                    inner.push(' ');
                } else {
                    inner.push(c);
                }
            }
            let mut parts: Vec<String> = Vec::new();
            for p in inner.split_whitespace() {
                parts.push(p.to_string());
            }
            if parts.len() < 3 {
                return false;
            }
            let name = name.trim().to_string();
            if name == "rgb" || name == "rgba" {
                let r = css_channel(&parts[0]);
                let g = css_channel(&parts[1]);
                let b = css_channel(&parts[2]);
                self.r = r.min(1.0);
                self.g = g.min(1.0);
                self.b = b.min(1.0);
                self.to_working(space);
                return true;
            }
            if name == "hsl" || name == "hsla" {
                let h = parse_number(&parts[0]) / 360.0;
                let sat = parse_number(&parts[1]) / 100.0;
                let light = parse_number(&parts[2]) / 100.0;
                self.set_hsl(h, sat, light, space);
                return true;
            }
            return false;
        }
        let hex = css_name_hex(&s);
        if hex < 0 {
            return false;
        }
        self.set_hex(hex, space);
        true
    }

    /// A CSS colour name; answers false for an unknown one.
    pub fn set_color_name(&mut self, name: &str, space: ColorSpace) -> bool {
        let hex = css_name_hex(&name.to_lowercase());
        if hex < 0 {
            return false;
        }
        self.set_hex(hex, space);
        true
    }

    pub fn copy(&mut self, c: &Color) -> &mut Color {
        self.r = c.r;
        self.g = c.g;
        self.b = c.b;
        self
    }

    pub fn copy_srgb_to_linear(&mut self, c: &Color) -> &mut Color {
        self.r = srgb_to_linear(c.r);
        self.g = srgb_to_linear(c.g);
        self.b = srgb_to_linear(c.b);
        self
    }

    pub fn copy_linear_to_srgb(&mut self, c: &Color) -> &mut Color {
        self.r = linear_to_srgb(c.r);
        self.g = linear_to_srgb(c.g);
        self.b = linear_to_srgb(c.b);
        self
    }

    pub fn convert_srgb_to_linear(&mut self) -> &mut Color {
        let c = *self;
        self.copy_srgb_to_linear(&c)
    }

    pub fn convert_linear_to_srgb(&mut self) -> &mut Color {
        let c = *self;
        self.copy_linear_to_srgb(&c)
    }

    pub fn get_hex(&self, space: ColorSpace) -> i64 {
        let c = self.in_space(space);
        let r = js_round(clamp(c.r * 255.0, 0.0, 255.0)) as i64;
        let g = js_round(clamp(c.g * 255.0, 0.0, 255.0)) as i64;
        let b = js_round(clamp(c.b * 255.0, 0.0, 255.0)) as i64;
        r * 65536 + g * 256 + b
    }

    /// Six lower-case hex digits, as three.js's `getHexString`.
    pub fn get_hex_string(&self, space: ColorSpace) -> String {
        let hex = self.get_hex(space);
        let digits: Vec<char> = "0123456789abcdef".chars().collect();
        let mut out = String::new();
        let mut shift: i64 = 20;
        while shift >= 0 {
            out.push(digits[((hex >> shift) & 15) as usize]);
            shift -= 4;
        }
        out
    }

    pub fn get_hsl(&self, space: ColorSpace) -> Hsl {
        let c = self.in_space(space);
        let r = c.r;
        let g = c.g;
        let b = c.b;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (min + max) / 2.0;
        if min == max {
            return Hsl { h: 0.0, s: 0.0, l: lightness };
        }
        let delta = max - min;
        let saturation = if lightness <= 0.5 { delta / (max + min) } else { delta / (2.0 - max - min) };
        let hue: f64;
        if max == r {
            hue = (g - b) / delta + if g < b { 6.0 } else { 0.0 };
        } else if max == g {
            hue = (b - r) / delta + 2.0;
        } else {
            hue = (r - g) / delta + 4.0;
        }
        Hsl { h: hue / 6.0, s: saturation, l: lightness }
    }

    pub fn get_rgb(&self, space: ColorSpace) -> Color {
        self.in_space(space)
    }

    /// `rgb(r,g,b)` with 0..255 sRGB channels.
    pub fn get_style(&self) -> String {
        let c = self.in_space(ColorSpace::Srgb);
        let r = js_round(c.r * 255.0) as i64;
        let g = js_round(c.g * 255.0) as i64;
        let b = js_round(c.b * 255.0) as i64;
        format!("rgb({},{},{})", r, g, b)
    }

    pub fn offset_hsl(&mut self, h: f64, s: f64, l: f64) -> &mut Color {
        let hsl = self.get_hsl(ColorSpace::Linear);
        self.set_hsl(hsl.h + h, hsl.s + s, hsl.l + l, ColorSpace::Linear)
    }

    pub fn add(&mut self, c: &Color) -> &mut Color {
        self.r += c.r;
        self.g += c.g;
        self.b += c.b;
        self
    }

    pub fn add_colors(&mut self, a: &Color, b: &Color) -> &mut Color {
        self.r = a.r + b.r;
        self.g = a.g + b.g;
        self.b = a.b + b.b;
        self
    }

    pub fn add_scalar(&mut self, s: f64) -> &mut Color {
        self.r += s;
        self.g += s;
        self.b += s;
        self
    }

    /// Subtracts, clamping each component at zero.
    pub fn sub(&mut self, c: &Color) -> &mut Color {
        self.r = (self.r - c.r).max(0.0);
        self.g = (self.g - c.g).max(0.0);
        self.b = (self.b - c.b).max(0.0);
        self
    }

    pub fn multiply(&mut self, c: &Color) -> &mut Color {
        self.r *= c.r;
        self.g *= c.g;
        self.b *= c.b;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Color {
        self.r *= s;
        self.g *= s;
        self.b *= s;
        self
    }

    pub fn lerp(&mut self, c: &Color, alpha: f64) -> &mut Color {
        self.r += (c.r - self.r) * alpha;
        self.g += (c.g - self.g) * alpha;
        self.b += (c.b - self.b) * alpha;
        self
    }

    pub fn lerp_colors(&mut self, a: &Color, b: &Color, alpha: f64) -> &mut Color {
        self.r = a.r + (b.r - a.r) * alpha;
        self.g = a.g + (b.g - a.g) * alpha;
        self.b = a.b + (b.b - a.b) * alpha;
        self
    }

    pub fn lerp_hsl(&mut self, c: &Color, alpha: f64) -> &mut Color {
        let a = self.get_hsl(ColorSpace::Linear);
        let b = c.get_hsl(ColorSpace::Linear);
        let h = lerp(a.h, b.h, alpha);
        let s = lerp(a.s, b.s, alpha);
        let l = lerp(a.l, b.l, alpha);
        self.set_hsl(h, s, l, ColorSpace::Linear)
    }

    pub fn set_from_vector3(&mut self, v: &Vector3) -> &mut Color {
        self.r = v.x;
        self.g = v.y;
        self.b = v.z;
        self
    }

    pub fn apply_matrix3(&mut self, m: &Matrix3) -> &mut Color {
        let r = self.r;
        let g = self.g;
        let b = self.b;
        let e = m.elements;
        self.r = e[0] * r + e[3] * g + e[6] * b;
        self.g = e[1] * r + e[4] * g + e[7] * b;
        self.b = e[2] * r + e[5] * g + e[8] * b;
        self
    }

    pub fn equals(&self, c: &Color) -> bool {
        self.r == c.r && self.g == c.g && self.b == c.b
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Color {
        self.r = array[offset];
        self.g = array[offset + 1];
        self.b = array[offset + 2];
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        vec![self.r, self.g, self.b]
    }
}

fn hex_digit(c: char) -> i64 {
    let digits: Vec<char> = "0123456789abcdef".chars().collect();
    for i in 0..16 {
        if digits[i] == c {
            return i as i64;
        }
    }
    -1
}

/// A number at the start of `s` (digits, sign, point, exponent).
pub fn parse_number(s: &str) -> f64 {
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' {
            num.push(c);
        } else {
            break;
        }
    }
    num.parse::<f64>().unwrap_or(0.0)
}

/// An rgb() channel, 0..255 or a percentage, as 0..1.
fn css_channel(s: &str) -> f64 {
    let v = parse_number(s);
    if s.ends_with('%') {
        v / 100.0
    } else {
        v / 255.0
    }
}

/// The sRGB hex of a CSS colour name, or -1.
pub fn css_name_hex(name: &str) -> i64 {
    for entry in CSS_NAMES.split(' ') {
        let mut key = String::new();
        let mut hex: i64 = 0;
        let mut after = false;
        for c in entry.chars() {
            if c == ':' {
                after = true;
            } else if after {
                hex = hex * 16 + hex_digit(c.to_ascii_lowercase());
            } else {
                key.push(c);
            }
        }
        if key == name {
            return hex;
        }
    }
    -1
}

/// The CSS Color Module's named colours, `name:rrggbb`.
pub const CSS_NAMES: &str = "aliceblue:F0F8FF antiquewhite:FAEBD7 aqua:00FFFF aquamarine:7FFFD4 azure:F0FFFF beige:F5F5DC bisque:FFE4C4 black:000000 blanchedalmond:FFEBCD blue:0000FF blueviolet:8A2BE2 brown:A52A2A burlywood:DEB887 cadetblue:5F9EA0 chartreuse:7FFF00 chocolate:D2691E coral:FF7F50 cornflowerblue:6495ED cornsilk:FFF8DC crimson:DC143C cyan:00FFFF darkblue:00008B darkcyan:008B8B darkgoldenrod:B8860B darkgray:A9A9A9 darkgreen:006400 darkgrey:A9A9A9 darkkhaki:BDB76B darkmagenta:8B008B darkolivegreen:556B2F darkorange:FF8C00 darkorchid:9932CC darkred:8B0000 darksalmon:E9967A darkseagreen:8FBC8F darkslateblue:483D8B darkslategray:2F4F4F darkslategrey:2F4F4F darkturquoise:00CED1 darkviolet:9400D3 deeppink:FF1493 deepskyblue:00BFFF dimgray:696969 dimgrey:696969 dodgerblue:1E90FF firebrick:B22222 floralwhite:FFFAF0 forestgreen:228B22 fuchsia:FF00FF gainsboro:DCDCDC ghostwhite:F8F8FF gold:FFD700 goldenrod:DAA520 gray:808080 green:008000 greenyellow:ADFF2F grey:808080 honeydew:F0FFF0 hotpink:FF69B4 indianred:CD5C5C indigo:4B0082 ivory:FFFFF0 khaki:F0E68C lavender:E6E6FA lavenderblush:FFF0F5 lawngreen:7CFC00 lemonchiffon:FFFACD lightblue:ADD8E6 lightcoral:F08080 lightcyan:E0FFFF lightgoldenrodyellow:FAFAD2 lightgray:D3D3D3 lightgreen:90EE90 lightgrey:D3D3D3 lightpink:FFB6C1 lightsalmon:FFA07A lightseagreen:20B2AA lightskyblue:87CEFA lightslategray:778899 lightslategrey:778899 lightsteelblue:B0C4DE lightyellow:FFFFE0 lime:00FF00 limegreen:32CD32 linen:FAF0E6 magenta:FF00FF maroon:800000 mediumaquamarine:66CDAA mediumblue:0000CD mediumorchid:BA55D3 mediumpurple:9370DB mediumseagreen:3CB371 mediumslateblue:7B68EE mediumspringgreen:00FA9A mediumturquoise:48D1CC mediumvioletred:C71585 midnightblue:191970 mintcream:F5FFFA mistyrose:FFE4E1 moccasin:FFE4B5 navajowhite:FFDEAD navy:000080 oldlace:FDF5E6 olive:808000 olivedrab:6B8E23 orange:FFA500 orangered:FF4500 orchid:DA70D6 palegoldenrod:EEE8AA palegreen:98FB98 paleturquoise:AFEEEE palevioletred:DB7093 papayawhip:FFEFD5 peachpuff:FFDAB9 peru:CD853F pink:FFC0CB plum:DDA0DD powderblue:B0E0E6 purple:800080 rebeccapurple:663399 red:FF0000 rosybrown:BC8F8F royalblue:4169E1 saddlebrown:8B4513 salmon:FA8072 sandybrown:F4A460 seagreen:2E8B57 seashell:FFF5EE sienna:A0522D silver:C0C0C0 skyblue:87CEEB slateblue:6A5ACD slategray:708090 slategrey:708090 snow:FFFAFA springgreen:00FF7F steelblue:4682B4 tan:D2B48C teal:008080 thistle:D8BFD8 tomato:FF6347 turquoise:40E0D0 violet:EE82EE wheat:F5DEB3 white:FFFFFF whitesmoke:F5F5F5 yellow:FFFF00 yellowgreen:9ACD32";
